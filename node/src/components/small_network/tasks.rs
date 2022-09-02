//! Tasks run by the component.

use std::{
    fmt::Display,
    io,
    net::SocketAddr,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc, Weak},
};

use bincode::Options;
use futures::{
    future::{self, Either},
    stream::SplitStream,
    SinkExt, Stream, StreamExt, TryStreamExt,
};
use muxink::{
    codec::{
        bincode::{BincodeDecoder, BincodeEncoder},
        length_delimited::LengthDelimited,
        TranscodingIoError, TranscodingStream,
    },
    io::{FrameReader, FrameWriter},
};
use openssl::{
    pkey::{PKey, Private},
    ssl::Ssl,
};
use prometheus::IntGauge;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    net::TcpStream,
    sync::{mpsc::UnboundedReceiver, watch, Semaphore},
};
use tokio_openssl::SslStream;
use tracing::{
    debug, error, error_span,
    field::{self, Empty},
    info, trace, warn, Instrument, Span,
};

use casper_types::{ProtocolVersion, TimeDiff};

use super::{
    chain_info::ChainInfo,
    counting_format::{ConnectionId, Role},
    error::{ConnectionError, MessageReaderError},
    event::{IncomingConnection, OutgoingConnection},
    handshake::{negotiate_handshake, HandshakeOutcome},
    limiter::LimiterHandle,
    message::ConsensusKeyPair,
    BincodeFormat, EstimatorWeights, Event, FromIncoming, Message, Metrics, Payload, Transport,
};

use crate::{
    components::small_network::{IncomingStream, OutgoingSink},
    effect::{requests::NetworkRequest, AutoClosingResponder, EffectBuilder},
    reactor::{EventQueueHandle, QueueKind},
    tls::{self, TlsCert},
    types::NodeId,
    utils::display_error,
};

/// An item on the internal outgoing message queue.
///
/// Contains a reference counted message and an optional responder to call once the message has been
/// successfully handed over to the kernel for sending.
pub(super) type MessageQueueItem<P> = (Arc<Message<P>>, Option<AutoClosingResponder<()>>);

/// Low-level TLS connection function.
///
/// Performs the actual TCP+TLS connection setup.
async fn tls_connect<REv>(
    context: &NetworkContext<REv>,
    peer_addr: SocketAddr,
) -> Result<(NodeId, Transport), ConnectionError>
where
    REv: 'static,
{
    let stream = TcpStream::connect(peer_addr)
        .await
        .map_err(ConnectionError::TcpConnection)?;

    stream
        .set_nodelay(true)
        .map_err(ConnectionError::TcpNoDelay)?;

    let mut transport = tls::create_tls_connector(context.our_cert.as_x509(), &context.secret_key)
        .and_then(|connector| connector.configure())
        .and_then(|mut config| {
            config.set_verify_hostname(false);
            config.into_ssl("this-will-not-be-checked.example.com")
        })
        .and_then(|ssl| SslStream::new(ssl, stream))
        .map_err(ConnectionError::TlsInitialization)?;

    SslStream::connect(Pin::new(&mut transport))
        .await
        .map_err(ConnectionError::TlsHandshake)?;

    let peer_cert = transport
        .ssl()
        .peer_certificate()
        .ok_or(ConnectionError::NoPeerCertificate)?;

    let peer_id = NodeId::from(
        tls::validate_cert(peer_cert)
            .map_err(ConnectionError::PeerCertificateInvalid)?
            .public_key_fingerprint(),
    );

    Ok((peer_id, transport))
}

/// Initiates a TLS connection to a remote address.
pub(super) async fn connect_outgoing<P, REv>(
    context: Arc<NetworkContext<REv>>,
    peer_addr: SocketAddr,
) -> OutgoingConnection<P>
where
    REv: 'static,
    P: Payload,
{
    let (peer_id, transport) = match tls_connect(&context, peer_addr).await {
        Ok(value) => value,
        Err(error) => return OutgoingConnection::FailedEarly { peer_addr, error },
    };

    // Register the `peer_id` on the [`Span`].
    Span::current().record("peer_id", &field::display(peer_id));

    if peer_id == context.our_id {
        info!("incoming loopback connection");
        return OutgoingConnection::Loopback { peer_addr };
    }

    debug!("Outgoing TLS connection established");

    // Setup connection id and framed transport.
    let connection_id = ConnectionId::from_connection(transport.ssl(), context.our_id, peer_id);

    // Negotiate the handshake, concluding the incoming connection process.
    match negotiate_handshake::<P, _>(&context, transport, connection_id).await {
        Ok(HandshakeOutcome {
            transport,
            public_addr,
            peer_consensus_public_key,
            is_peer_syncing: is_syncing,
        }) => {
            if let Some(ref public_key) = peer_consensus_public_key {
                Span::current().record("validator_id", &field::display(public_key));
            }

            if public_addr != peer_addr {
                // We don't need the `public_addr`, as we already connected, but warn anyway.
                warn!(%public_addr, %peer_addr, "peer advertises a different public address than what we connected to");
            }

            // `rust-openssl` does not support the futures 0.3 `AsyncWrite` trait (it uses the
            // tokio built-in version instead). The compat layer fixes that.
            let compat_stream =
                tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(transport);

            use muxink::SinkMuxExt;
            let sink: OutgoingSink<P> = FrameWriter::new(LengthDelimited, compat_stream)
                .with_transcoder(BincodeEncoder::new());

            OutgoingConnection::Established {
                peer_addr,
                peer_id,
                peer_consensus_public_key,
                sink,
                is_syncing,
            }
        }
        Err(error) => OutgoingConnection::Failed {
            peer_addr,
            peer_id,
            error,
        },
    }
}

/// A context holding all relevant information for networking communication shared across tasks.
pub(crate) struct NetworkContext<REv>
where
    REv: 'static,
{
    /// Event queue handle.
    pub(super) event_queue: EventQueueHandle<REv>,
    /// Our own [`NodeId`].
    pub(super) our_id: NodeId,
    /// TLS certificate associated with this node's identity.
    pub(super) our_cert: Arc<TlsCert>,
    /// Secret key associated with `our_cert`.
    pub(super) secret_key: Arc<PKey<Private>>,
    /// Weak reference to the networking metrics shared by all sender/receiver tasks.
    pub(super) net_metrics: Weak<Metrics>,
    /// Chain info extract from chainspec.
    pub(super) chain_info: ChainInfo,
    /// Our own public listening address.
    pub(super) public_addr: SocketAddr,
    /// Optional set of consensus keys, to identify as a validator during handshake.
    pub(super) consensus_keys: Option<ConsensusKeyPair>,
    /// Timeout for handshake completion.
    pub(super) handshake_timeout: TimeDiff,
    /// Weights to estimate payloads with.
    pub(super) payload_weights: EstimatorWeights,
    /// The protocol version at which (or under) tarpitting is enabled.
    pub(super) tarpit_version_threshold: Option<ProtocolVersion>,
    /// If tarpitting is enabled, duration for which connections should be kept open.
    pub(super) tarpit_duration: TimeDiff,
    /// The chance, expressed as a number between 0.0 and 1.0, of triggering the tarpit.
    pub(super) tarpit_chance: f32,
    /// Maximum number of demands allowed to be running at once. If 0, no limit is enforced.
    pub(super) max_in_flight_demands: usize,
    /// Flag indicating whether this node is syncing.
    pub(super) is_syncing: AtomicBool,
}

/// Handles an incoming connection.
///
/// Sets up a TLS stream and performs the protocol handshake.
async fn handle_incoming<P, REv>(
    context: Arc<NetworkContext<REv>>,
    stream: TcpStream,
    peer_addr: SocketAddr,
) -> IncomingConnection<P>
where
    REv: From<Event<P>> + 'static,
    P: Payload,
    for<'de> P: Serialize + Deserialize<'de>,
    for<'de> Message<P>: Serialize + Deserialize<'de>,
{
    let (peer_id, transport) =
        match server_setup_tls(stream, &context.our_cert, &context.secret_key).await {
            Ok(value) => value,
            Err(error) => {
                return IncomingConnection::FailedEarly { peer_addr, error };
            }
        };

    // Register the `peer_id` on the [`Span`] for logging the ID from here on out.
    Span::current().record("peer_id", &field::display(peer_id));

    if peer_id == context.our_id {
        info!("incoming loopback connection");
        return IncomingConnection::Loopback;
    }

    debug!("Incoming TLS connection established");

    // Setup connection id and framed transport.
    let connection_id = ConnectionId::from_connection(transport.ssl(), context.our_id, peer_id);

    // Negotiate the handshake, concluding the incoming connection process.
    match negotiate_handshake::<P, _>(&context, transport, connection_id).await {
        Ok(HandshakeOutcome {
            transport,
            public_addr,
            peer_consensus_public_key,
            is_peer_syncing: _,
        }) => {
            if let Some(ref public_key) = peer_consensus_public_key {
                Span::current().record("validator_id", &field::display(public_key));
            }

            // TODO: Removal of `CountingTransport` here means some functionality has to be restored.

            // `rust-openssl` does not support the futures 0.3 `AsyncRead` trait (it uses the
            // tokio built-in version instead). The compat layer fixes that.
            use muxink::StreamMuxExt; // TODO: Move, once methods are renamed.
            let compat_stream = tokio_util::compat::TokioAsyncReadCompatExt::compat(transport);

            // TODO: We need to split the stream here eventually. Right now, this is safe since the
            //       reader only uses one direction.
            let stream: IncomingStream<P> = FrameReader::new(LengthDelimited, compat_stream, 4096)
                .and_then_transcode(BincodeDecoder::new());

            IncomingConnection::Established {
                peer_addr,
                public_addr,
                peer_id,
                peer_consensus_public_key,
                stream,
            }
        }
        Err(error) => IncomingConnection::Failed {
            peer_addr,
            peer_id,
            error,
        },
    }
}

/// Server-side TLS setup.
///
/// This function groups the TLS setup into a convenient function, enabling the `?` operator.
pub(super) async fn server_setup_tls(
    stream: TcpStream,
    cert: &TlsCert,
    secret_key: &PKey<Private>,
) -> Result<(NodeId, Transport), ConnectionError> {
    let mut tls_stream = tls::create_tls_acceptor(cert.as_x509().as_ref(), secret_key.as_ref())
        .and_then(|ssl_acceptor| Ssl::new(ssl_acceptor.context()))
        .and_then(|ssl| SslStream::new(ssl, stream))
        .map_err(ConnectionError::TlsInitialization)?;

    SslStream::accept(Pin::new(&mut tls_stream))
        .await
        .map_err(ConnectionError::TlsHandshake)?;

    // We can now verify the certificate.
    let peer_cert = tls_stream
        .ssl()
        .peer_certificate()
        .ok_or(ConnectionError::NoPeerCertificate)?;

    Ok((
        NodeId::from(
            tls::validate_cert(peer_cert)
                .map_err(ConnectionError::PeerCertificateInvalid)?
                .public_key_fingerprint(),
        ),
        tls_stream,
    ))
}

/// Runs the server core acceptor loop.
pub(super) async fn server<P, REv>(
    context: Arc<NetworkContext<REv>>,
    listener: tokio::net::TcpListener,
    mut shutdown_receiver: watch::Receiver<()>,
) where
    REv: From<Event<P>> + Send,
    P: Payload,
{
    // The server task is a bit tricky, since it has to wait on incoming connections while at the
    // same time shut down if the networking component is dropped, otherwise the TCP socket will
    // stay open, preventing reuse.

    // We first create a future that never terminates, handling incoming connections:
    let accept_connections = async {
        loop {
            // We handle accept errors here, since they can be caused by a temporary resource
            // shortage or the remote side closing the connection while it is waiting in
            // the queue.
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    // The span setup here is used throughout the entire lifetime of the connection.
                    let span =
                        error_span!("incoming", %peer_addr, peer_id=Empty, validator_id=Empty);

                    let context = context.clone();
                    let handler_span = span.clone();
                    tokio::spawn(
                        async move {
                            let incoming =
                                handle_incoming(context.clone(), stream, peer_addr).await;
                            context
                                .event_queue
                                .schedule(
                                    Event::IncomingConnection {
                                        incoming: Box::new(incoming),
                                        span,
                                    },
                                    QueueKind::NetworkIncoming,
                                )
                                .await;
                        }
                        .instrument(handler_span),
                    );
                }

                // TODO: Handle resource errors gracefully.
                //       In general, two kinds of errors occur here: Local resource exhaustion,
                //       which should be handled by waiting a few milliseconds, or remote connection
                //       errors, which can be dropped immediately.
                //
                //       The code in its current state will consume 100% CPU if local resource
                //       exhaustion happens, as no distinction is made and no delay introduced.
                Err(ref err) => {
                    warn!(%context.our_id, err=display_error(err), "dropping incoming connection during accept")
                }
            }
        }
    };

    let shutdown_messages = async move { while shutdown_receiver.changed().await.is_ok() {} };

    // Now we can wait for either the `shutdown` channel's remote end to do be dropped or the
    // infinite loop to terminate, which never happens.
    match future::select(Box::pin(shutdown_messages), Box::pin(accept_connections)).await {
        Either::Left(_) => info!(
            %context.our_id,
            "shutting down socket, no longer accepting incoming connections"
        ),
        Either::Right(_) => unreachable!(),
    }
}

/// Network message reader.
///
/// Schedules all received messages until the stream is closed or an error occurs.
pub(super) async fn message_reader<REv, P>(
    context: Arc<NetworkContext<REv>>,
    mut stream: IncomingStream<P>,
    limiter: Box<dyn LimiterHandle>,
    mut close_incoming_receiver: watch::Receiver<()>,
    peer_id: NodeId,
    span: Span,
) -> Result<(), MessageReaderError>
where
    P: DeserializeOwned + Send + Display + Payload,
    REv: From<Event<P>> + FromIncoming<P> + From<NetworkRequest<P>> + Send,
{
    let demands_in_flight = Arc::new(Semaphore::new(context.max_in_flight_demands));

    let read_messages =
        async move {
            while let Some(msg_result) = stream.next().await {
                match msg_result {
                    Ok(msg) => {
                        trace!(%msg, "message received");

                        let effect_builder = EffectBuilder::new(context.event_queue);

                        match msg.try_into_demand(effect_builder, peer_id) {
                            Ok((event, wait_for_response)) => {
                                // Note: For now, demands bypass the limiter, as we expect the
                                //       backpressure to handle this instead.

                                // Acquire a permit. If we are handling too many demands at this
                                // time, this will block, halting the processing of new message,
                                // thus letting the peer they have reached their maximum allowance.
                                let in_flight = demands_in_flight
                                    .clone()
                                    .acquire_owned()
                                    .await
                                    // Note: Since the semaphore is reference counted, it must
                                    //       explicitly be closed for acquisition to fail, which we
                                    //       never do. If this happens, there is a bug in the code;
                                    //       we exit with an error and close the connection.
                                    .map_err(|_| MessageReaderError::UnexpectedSemaphoreClose)?;

                                Metrics::record_trie_request_start(&context.net_metrics);

                                let net_metrics = context.net_metrics.clone();
                                // Spawn a future that will eventually send the returned message. It
                                // will essentially buffer the response.
                                tokio::spawn(async move {
                                    if let Some(payload) = wait_for_response.await {
                                        // Send message and await its return. `send_message` should
                                        // only return when the message has been buffered, if the
                                        // peer is not accepting data, we will block here until the
                                        // send buffer has sufficient room.
                                        effect_builder.send_message(peer_id, payload).await;

                                        // Note: We could short-circuit the event queue here and
                                        //       directly insert into the outgoing message queue,
                                        //       which may be potential performance improvement.
                                    }

                                    // Missing else: The handler of the demand did not deem it
                                    // worthy a response. Just drop it.

                                    // After we have either successfully buffered the message for
                                    // sending, failed to do so or did not have a message to send
                                    // out, we consider the request handled and free up the permit.
                                    Metrics::record_trie_request_end(&net_metrics);
                                    drop(in_flight);
                                });

                                // Schedule the created event.
                                context
                                    .event_queue
                                    .schedule::<REv>(event, QueueKind::NetworkDemand)
                                    .await;
                            }
                            Err(msg) => {
                                // We've received a non-demand message. Ensure we have the proper amount
                                // of resources, then push it to the reactor.
                                limiter
                                    .request_allowance(msg.payload_incoming_resource_estimate(
                                        &context.payload_weights,
                                    ))
                                    .await;

                                let queue_kind = if msg.is_low_priority() {
                                    QueueKind::NetworkLowPriority
                                } else {
                                    QueueKind::NetworkIncoming
                                };

                                context
                                    .event_queue
                                    .schedule(
                                        Event::IncomingMessage {
                                            peer_id: Box::new(peer_id),
                                            msg: Box::new(msg),
                                            span: span.clone(),
                                        },
                                        queue_kind,
                                    )
                                    .await;
                            }
                        }
                    }
                    Err(err) => {
                        // TODO: Consider not logging the error here, as it will be logged in the
                        //       same span in the component proper.
                        warn!(
                            err = display_error(&err),
                            "receiving message failed, closing connection"
                        );
                        return Err(MessageReaderError::ReceiveError(Box::new(err)));
                    }
                }
            }
            Ok(())
        };

    let shutdown_messages = async move { while close_incoming_receiver.changed().await.is_ok() {} };

    // Now we can wait for either the `shutdown` channel's remote end to do be dropped or the
    // while loop to terminate.
    match future::select(Box::pin(shutdown_messages), Box::pin(read_messages)).await {
        Either::Left(_) => info!("shutting down incoming connection message reader"),
        Either::Right(_) => (),
    }

    Ok(())
}

/// Network message sender.
///
/// Reads from a channel and sends all messages, until the stream is closed or an error occurs.
pub(super) async fn message_sender<P>(
    mut queue: UnboundedReceiver<MessageQueueItem<P>>,
    mut sink: OutgoingSink<P>,
    limiter: Box<dyn LimiterHandle>,
    counter: IntGauge,
) where
    P: Payload,
{
    while let Some((message, opt_responder)) = queue.recv().await {
        counter.dec();

        let estimated_wire_size = match BincodeFormat::default().0.serialized_size(&*message) {
            Ok(size) => size as u32,
            Err(error) => {
                error!(
                    error = display_error(&error),
                    "failed to get serialized size of outgoing message, closing outgoing connection"
                );
                break;
            }
        };
        limiter.request_allowance(estimated_wire_size).await;

        let mut outcome = sink.send(message).await;

        // Notify via responder that the message has been buffered by the kernel.
        if let Some(auto_closing_responder) = opt_responder {
            // Since someone is interested in the message, flush the socket to ensure it was sent.
            outcome = outcome.and(sink.flush().await);
            auto_closing_responder.respond(()).await;
        }

        // We simply error-out if the sink fails, it means that our connection broke.
        if let Err(ref err) = outcome {
            info!(
                err = display_error(err),
                "message send failed, closing outgoing connection"
            );

            // To ensure, metrics are up to date, we close the queue and drain it.
            queue.close();
            while queue.recv().await.is_some() {
                counter.dec();
            }

            break;
        };
    }
}
