//! Stream multiplexing
//!
//! Multiplexes multiple sinks into a single one, allowing no more than one frame to be buffered for
//! each to avoid starvation or flooding.

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use bytes::Buf;
use futures::{future::FusedFuture, Future, FutureExt, Sink, SinkExt};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio_util::sync::ReusableBoxFuture;

use crate::ImmediateFrame;

pub type ChannelPrefixedFrame<F> = bytes::buf::Chain<ImmediateFrame<[u8; 1]>, F>;

/// A frame multiplexer.
///
/// Typically the multiplexer is not used directly, but used to spawn multiplexing handles.
pub struct Multiplexer<S> {
    sink: Arc<Mutex<Option<S>>>,
}

impl<S> Multiplexer<S>
where
    S: Send + 'static,
{
    /// Create a handle for a specific multiplexer channel on this multiplexer.
    pub fn get_channel_handle(self: Arc<Self>, channel: u8) -> MultiplexerHandle<S> {
        MultiplexerHandle {
            multiplexer: self.clone(),
            slot: channel,
            lock_future: ReusableBoxFuture::new(mk_lock_future(self)),
            guard: None,
        }
    }
}

type SinkGuard<S> = OwnedMutexGuard<Option<S>>;

trait FuseFuture: Future + FusedFuture + Send {}
impl<T> FuseFuture for T where T: Future + FusedFuture + Send {}

fn mk_lock_future<S>(
    multiplexer: Arc<Multiplexer<S>>,
) -> impl futures::Future<Output = tokio::sync::OwnedMutexGuard<std::option::Option<S>>> {
    multiplexer.sink.clone().lock_owned()
}

pub struct MultiplexerHandle<S> {
    multiplexer: Arc<Multiplexer<S>>,
    slot: u8,

    // TODO NEW IDEA: Maybe we can create the lock future right away, but never poll it? Then use
    //                the `ReusableBoxFuture` and always create a new one right away? Need to check
    //                source of `lock`. Write a test for this?
    // lock_future: Box<dyn Future<Output = SinkGuard<S>> + Send + 'static>,
    lock_future: ReusableBoxFuture<'static, SinkGuard<S>>,
    guard: Option<SinkGuard<S>>,
}

impl<S> MultiplexerHandle<S>
where
    S: Send + 'static,
{
    fn assume_get_sink(&mut self) -> &mut S {
        match self.guard {
            Some(ref mut guard) => {
                let mref = guard.as_mut().expect("TODO: guard disappeard");
                mref
            }
            None => todo!("assumed sink, but no sink"),
        }
    }
}

impl<F, S> Sink<F> for MultiplexerHandle<S>
where
    S: Sink<ChannelPrefixedFrame<F>> + Unpin + Send + 'static,
    F: Buf,
{
    type Error = <S as Sink<ChannelPrefixedFrame<F>>>::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.guard.is_none() {
            // We do not hold the guard at the moment, so attempt to acquire it.
            match self.lock_future.poll_unpin(cx) {
                Poll::Ready(guard) => {
                    // It is our turn: Save the guard and prepare another locking future for later,
                    // which will not attempt to lock until first polled.
                    let _ = self.guard.insert(guard);
                    let multiplexer = self.multiplexer.clone();
                    self.lock_future.set(mk_lock_future(multiplexer));
                }
                Poll::Pending => {
                    // The lock could not be acquired yet.
                    return Poll::Pending;
                }
            }
        }

        // At this point we have acquired the lock, now our only job is to stuff data into the sink.
        self.assume_get_sink().poll_ready_unpin(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: F) -> Result<(), Self::Error> {
        let prefixed = ImmediateFrame::from(self.slot).chain(item);
        self.assume_get_sink().start_send_unpin(prefixed)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Obtain the flush result, then release the sink lock.
        match self.assume_get_sink().poll_flush_unpin(cx) {
            Poll::Ready(Ok(())) => {
                // Acquire wait list lock to update it.
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(_)) => {
                todo!("handle error")
            }

            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Simply close? Note invariants, possibly checking them in debug mode.
        todo!()
    }
}
