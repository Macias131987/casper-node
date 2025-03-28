//! Key types.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use core::{
    convert::TryFrom,
    fmt::{self, Debug, Display, Formatter},
    str::FromStr,
};

use blake2::{
    digest::{Update, VariableOutput},
    VarBlake2b,
};
#[cfg(feature = "datasize")]
use datasize::DataSize;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use serde::{de::Error as SerdeError, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    account::{self, AccountHash, ACCOUNT_HASH_LENGTH},
    bytesrepr::{self, Error, FromBytes, ToBytes, U64_SERIALIZED_LENGTH},
    checksummed_hex,
    contract_wasm::ContractWasmHash,
    contracts::{ContractHash, ContractPackageHash},
    uref::{self, URef, URefAddr, UREF_SERIALIZED_LENGTH},
    DeployHash, EraId, Tagged, TransferAddr, TransferFromStrError, DEPLOY_HASH_LENGTH,
    TRANSFER_ADDR_LENGTH, UREF_ADDR_LENGTH,
};

const HASH_PREFIX: &str = "hash-";
const DEPLOY_INFO_PREFIX: &str = "deploy-";
const ERA_INFO_PREFIX: &str = "era-";
const BALANCE_PREFIX: &str = "balance-";
const BID_PREFIX: &str = "bid-";
const WITHDRAW_PREFIX: &str = "withdraw-";
const DICTIONARY_PREFIX: &str = "dictionary-";
const UNBOND_PREFIX: &str = "unbond-";
const SYSTEM_CONTRACT_REGISTRY_PREFIX: &str = "system-contract-registry-";
const CHAINSPEC_REGISTRY_PREFIX: &str = "chainspec-registry-";
const BLOCK_EFFECTS_ROOT_HASH_PREFIX: &str = "block-effects-root-hash-";
const DEPLOY_APPROVALS_ROOT_HASH_REGISTRY_PREFIX: &str = "deploy-approvals-root-hash-";

/// The number of bytes in a Blake2b hash
pub const BLAKE2B_DIGEST_LENGTH: usize = 32;
/// The number of bytes in a [`Key::Hash`].
pub const KEY_HASH_LENGTH: usize = 32;
/// The number of bytes in a [`Key::Transfer`].
pub const KEY_TRANSFER_LENGTH: usize = TRANSFER_ADDR_LENGTH;
/// The number of bytes in a [`Key::DeployInfo`].
pub const KEY_DEPLOY_INFO_LENGTH: usize = DEPLOY_HASH_LENGTH;
/// The number of bytes in a [`Key::Dictionary`].
pub const KEY_DICTIONARY_LENGTH: usize = 32;
/// The maximum length for a `dictionary_item_key`.
pub const DICTIONARY_ITEM_KEY_MAX_LENGTH: usize = 128;

const SYSTEM_CONTRACT_REGISTRY_KEY_BYTES: [u8; 32] = [0u8; 32];
const CHAINSPEC_REGISTRY_KEY_BYTES: [u8; 32] = [1u8; 32];
const KEY_ID_SERIALIZED_LENGTH: usize = 1;
// u8 used to determine the ID
const KEY_HASH_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + KEY_HASH_LENGTH;
const KEY_UREF_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + UREF_SERIALIZED_LENGTH;
const KEY_TRANSFER_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + KEY_TRANSFER_LENGTH;
const KEY_DEPLOY_INFO_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + KEY_DEPLOY_INFO_LENGTH;
const KEY_ERA_INFO_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + U64_SERIALIZED_LENGTH;
const KEY_BALANCE_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + UREF_ADDR_LENGTH;
const KEY_BID_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + KEY_HASH_LENGTH;
const KEY_WITHDRAW_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + KEY_HASH_LENGTH;
const KEY_UNBOND_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + KEY_HASH_LENGTH;
const KEY_DICTIONARY_SERIALIZED_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + KEY_DICTIONARY_LENGTH;
const KEY_SYSTEM_CONTRACT_REGISTRY_SERIALIZED_LENGTH: usize =
    KEY_ID_SERIALIZED_LENGTH + SYSTEM_CONTRACT_REGISTRY_KEY_BYTES.len();
const KEY_CHAINSPEC_REGISTRY_SERIALIZED_LENGTH: usize =
    KEY_ID_SERIALIZED_LENGTH + CHAINSPEC_REGISTRY_KEY_BYTES.len();
const KEY_BLOCK_EFFECTS_ROOT_HASH_LENGTH: usize = KEY_ID_SERIALIZED_LENGTH + U64_SERIALIZED_LENGTH;
const KEY_DEPLOY_APPROVALS_ROOT_HASH_SERIALIZED_LENGTH: usize =
    KEY_ID_SERIALIZED_LENGTH + U64_SERIALIZED_LENGTH;

/// An alias for [`Key`]s hash variant.
pub type HashAddr = [u8; KEY_HASH_LENGTH];

/// An alias for [`Key`]s dictionary variant.
pub type DictionaryAddr = [u8; KEY_DICTIONARY_LENGTH];

#[allow(missing_docs)]
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum KeyTag {
    Account = 0,
    Hash = 1,
    URef = 2,
    Transfer = 3,
    DeployInfo = 4,
    EraInfo = 5,
    Balance = 6,
    Bid = 7,
    Withdraw = 8,
    Dictionary = 9,
    SystemContractRegistry = 10,
    Unbond = 11,
    ChainspecRegistry = 12,
    BlockEffectsRootHash = 13,
    DeployApprovalsRootHash = 14,
}

/// The type under which data (e.g. [`CLValue`](crate::CLValue)s, smart contracts, user accounts)
/// are indexed on the network.
#[repr(C)]
#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
pub enum Key {
    /// A `Key` under which a user account is stored.
    Account(AccountHash),
    /// A `Key` under which a smart contract is stored and which is the pseudo-hash of the
    /// contract.
    Hash(HashAddr),
    /// A `Key` which is a [`URef`], under which most types of data can be stored.
    URef(URef),
    /// A `Key` under which we store a transfer.
    Transfer(TransferAddr),
    /// A `Key` under which we store a deploy info.
    DeployInfo(DeployHash),
    /// A `Key` under which we store an era info.
    EraInfo(EraId),
    /// A `Key` under which we store a purse balance.
    Balance(URefAddr),
    /// A `Key` under which we store bid information
    Bid(AccountHash),
    /// A `Key` under which we store withdraw information.
    Withdraw(AccountHash),
    /// A `Key` variant whose value is derived by hashing [`URef`]s address and arbitrary data.
    Dictionary(DictionaryAddr),
    /// A `Key` variant under which system contract hashes are stored.
    SystemContractRegistry,
    /// A `Key` under which we store unbond information.
    Unbond(AccountHash),
    /// A `Key` variant under which chainspec and other hashes are stored.
    ChainspecRegistry,
    /// A `Key` variant under which we store the root hash of a Merkle tree containing the
    /// execution results for the block at this height.
    BlockEffectsRootHash {
        /// The height of the block which these execution results correspond to.
        block_height: u64,
    },
    /// A `Key` variant under which we store the root hash of a Merkle tree containing the
    /// approvals for all the deploys in the block at this height.
    DeployApprovalsRootHash {
        /// The height of the block whose deploy approvals are hashed here.
        block_height: u64,
    },
}

/// Errors produced when converting a `String` into a `Key`.
#[derive(Debug)]
#[non_exhaustive]
pub enum FromStrError {
    /// Account parse error.
    Account(account::FromStrError),
    /// Hash parse error.
    Hash(String),
    /// URef parse error.
    URef(uref::FromStrError),
    /// Transfer parse error.
    Transfer(TransferFromStrError),
    /// DeployInfo parse error.
    DeployInfo(String),
    /// EraInfo parse error.
    EraInfo(String),
    /// Balance parse error.
    Balance(String),
    /// Bid parse error.
    Bid(String),
    /// Withdraw parse error.
    Withdraw(String),
    /// Dictionary parse error.
    Dictionary(String),
    /// System contract registry parse error.
    SystemContractRegistry(String),
    /// Unbond parse error.
    Unbond(String),
    /// Chainspec registry error.
    ChainspecRegistry(String),
    /// Execution results root hash error.
    BlockEffectsRootHash(String),
    /// DeployApprovalsRootHash parse error.
    DeployApprovalsRootHash(String),
    /// Unknown prefix.
    UnknownPrefix,
}

impl From<account::FromStrError> for FromStrError {
    fn from(error: account::FromStrError) -> Self {
        FromStrError::Account(error)
    }
}

impl From<TransferFromStrError> for FromStrError {
    fn from(error: TransferFromStrError) -> Self {
        FromStrError::Transfer(error)
    }
}

impl From<uref::FromStrError> for FromStrError {
    fn from(error: uref::FromStrError) -> Self {
        FromStrError::URef(error)
    }
}

impl Display for FromStrError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            FromStrError::Account(error) => write!(f, "account-key from string error: {}", error),
            FromStrError::Hash(error) => write!(f, "hash-key from string error: {}", error),
            FromStrError::URef(error) => write!(f, "uref-key from string error: {}", error),
            FromStrError::Transfer(error) => write!(f, "transfer-key from string error: {}", error),
            FromStrError::DeployInfo(error) => {
                write!(f, "deploy-info-key from string error: {}", error)
            }
            FromStrError::EraInfo(error) => write!(f, "era-info-key from string error: {}", error),
            FromStrError::Balance(error) => write!(f, "balance-key from string error: {}", error),
            FromStrError::Bid(error) => write!(f, "bid-key from string error: {}", error),
            FromStrError::Withdraw(error) => write!(f, "withdraw-key from string error: {}", error),
            FromStrError::Dictionary(error) => {
                write!(f, "dictionary-key from string error: {}", error)
            }
            FromStrError::SystemContractRegistry(error) => {
                write!(
                    f,
                    "system-contract-registry-key from string error: {}",
                    error
                )
            }
            FromStrError::Unbond(error) => {
                write!(f, "unbond-key from string error: {}", error)
            }
            FromStrError::ChainspecRegistry(error) => {
                write!(f, "chainspec-registry-key from string error: {}", error)
            }
            FromStrError::BlockEffectsRootHash(error) => {
                write!(
                    f,
                    "block-effects-root-hash-key from string error: {}",
                    error
                )
            }
            FromStrError::DeployApprovalsRootHash(error) => {
                write!(
                    f,
                    "deploy-approvals-root-hash-key from string error: {}",
                    error
                )
            }
            FromStrError::UnknownPrefix => write!(f, "unknown prefix for key"),
        }
    }
}

impl Key {
    // This method is not intended to be used by third party crates.
    #[doc(hidden)]
    pub fn type_string(&self) -> String {
        match self {
            Key::Account(_) => String::from("Key::Account"),
            Key::Hash(_) => String::from("Key::Hash"),
            Key::URef(_) => String::from("Key::URef"),
            Key::Transfer(_) => String::from("Key::Transfer"),
            Key::DeployInfo(_) => String::from("Key::DeployInfo"),
            Key::EraInfo(_) => String::from("Key::EraInfo"),
            Key::Balance(_) => String::from("Key::Balance"),
            Key::Bid(_) => String::from("Key::Bid"),
            Key::Withdraw(_) => String::from("Key::Unbond"),
            Key::Dictionary(_) => String::from("Key::Dictionary"),
            Key::Unbond(_) => String::from("Key::Unbond"),
            Key::SystemContractRegistry => String::from("Key::SystemContractRegistry"),
            Key::ChainspecRegistry => String::from("Key::ChainspecRegistry"),
            Key::BlockEffectsRootHash { .. } => String::from("Key::BlockEffectsRootHash"),
            Key::DeployApprovalsRootHash { .. } => String::from("Key::DeployApprovalsRootHash"),
        }
    }

    /// Returns the maximum size a [`Key`] can be serialized into.
    pub const fn max_serialized_length() -> usize {
        KEY_UREF_SERIALIZED_LENGTH
    }

    /// If `self` is of type [`Key::URef`], returns `self` with the
    /// [`AccessRights`](crate::AccessRights) stripped from the wrapped [`URef`], otherwise
    /// returns `self` unmodified.
    #[must_use]
    pub fn normalize(self) -> Key {
        match self {
            Key::URef(uref) => Key::URef(uref.remove_access_rights()),
            other => other,
        }
    }

    /// Returns a human-readable version of `self`, with the inner bytes encoded to Base16.
    pub fn to_formatted_string(self) -> String {
        match self {
            Key::Account(account_hash) => account_hash.to_formatted_string(),
            Key::Hash(addr) => format!("{}{}", HASH_PREFIX, base16::encode_lower(&addr)),
            Key::URef(uref) => uref.to_formatted_string(),
            Key::Transfer(transfer_addr) => transfer_addr.to_formatted_string(),
            Key::DeployInfo(addr) => {
                format!(
                    "{}{}",
                    DEPLOY_INFO_PREFIX,
                    base16::encode_lower(addr.as_bytes())
                )
            }
            Key::EraInfo(era_id) => {
                format!("{}{}", ERA_INFO_PREFIX, era_id.value())
            }
            Key::Balance(uref_addr) => {
                format!("{}{}", BALANCE_PREFIX, base16::encode_lower(&uref_addr))
            }
            Key::Bid(account_hash) => {
                format!("{}{}", BID_PREFIX, base16::encode_lower(&account_hash))
            }
            Key::Withdraw(account_hash) => {
                format!("{}{}", WITHDRAW_PREFIX, base16::encode_lower(&account_hash))
            }
            Key::Dictionary(dictionary_addr) => {
                format!(
                    "{}{}",
                    DICTIONARY_PREFIX,
                    base16::encode_lower(&dictionary_addr)
                )
            }
            Key::Unbond(account_hash) => {
                format!("{}{}", UNBOND_PREFIX, base16::encode_lower(&account_hash))
            }
            Key::SystemContractRegistry => {
                format!(
                    "{}{}",
                    SYSTEM_CONTRACT_REGISTRY_PREFIX,
                    base16::encode_lower(&SYSTEM_CONTRACT_REGISTRY_KEY_BYTES)
                )
            }
            Key::ChainspecRegistry => {
                format!(
                    "{}{}",
                    CHAINSPEC_REGISTRY_PREFIX,
                    base16::encode_lower(&CHAINSPEC_REGISTRY_KEY_BYTES)
                )
            }
            Key::BlockEffectsRootHash { block_height } => {
                format!("{}{}", BLOCK_EFFECTS_ROOT_HASH_PREFIX, block_height,)
            }
            Key::DeployApprovalsRootHash { block_height } => {
                format!(
                    "{}{}",
                    DEPLOY_APPROVALS_ROOT_HASH_REGISTRY_PREFIX, block_height
                )
            }
        }
    }

    /// Parses a string formatted as per `Self::to_formatted_string()` into a `Key`.
    pub fn from_formatted_str(input: &str) -> Result<Key, FromStrError> {
        match AccountHash::from_formatted_str(input) {
            Ok(account_hash) => return Ok(Key::Account(account_hash)),
            Err(account::FromStrError::InvalidPrefix) => {}
            Err(error) => return Err(error.into()),
        }

        if let Some(hex) = input.strip_prefix(HASH_PREFIX) {
            let addr = checksummed_hex::decode(hex)
                .map_err(|error| FromStrError::Hash(error.to_string()))?;
            let hash_addr = HashAddr::try_from(addr.as_ref())
                .map_err(|error| FromStrError::Hash(error.to_string()))?;
            return Ok(Key::Hash(hash_addr));
        }

        // We try this variant before `DeployInfo` as DEPLOY_INFO_PREFIX is a valid prefix of
        // DEPLOY_APPROVALS_ROOT_HASH_REGISTRY_PREFIX.
        if let Some(block_height_str) =
            input.strip_prefix(DEPLOY_APPROVALS_ROOT_HASH_REGISTRY_PREFIX)
        {
            let block_height = u64::from_str(block_height_str)
                .map_err(|error| FromStrError::DeployApprovalsRootHash(error.to_string()))?;
            return Ok(Key::DeployApprovalsRootHash { block_height });
        }

        if let Some(hex) = input.strip_prefix(DEPLOY_INFO_PREFIX) {
            let hash = checksummed_hex::decode(hex)
                .map_err(|error| FromStrError::DeployInfo(error.to_string()))?;
            let hash_array = <[u8; DEPLOY_HASH_LENGTH]>::try_from(hash.as_ref())
                .map_err(|error| FromStrError::DeployInfo(error.to_string()))?;
            return Ok(Key::DeployInfo(DeployHash::new(hash_array)));
        }

        match TransferAddr::from_formatted_str(input) {
            Ok(transfer_addr) => return Ok(Key::Transfer(transfer_addr)),
            Err(TransferFromStrError::InvalidPrefix) => {}
            Err(error) => return Err(error.into()),
        }

        match URef::from_formatted_str(input) {
            Ok(uref) => return Ok(Key::URef(uref)),
            Err(uref::FromStrError::InvalidPrefix) => {}
            Err(error) => return Err(error.into()),
        }

        if let Some(era_id_str) = input.strip_prefix(ERA_INFO_PREFIX) {
            let era_id = EraId::from_str(era_id_str)
                .map_err(|error| FromStrError::EraInfo(error.to_string()))?;
            return Ok(Key::EraInfo(era_id));
        }

        if let Some(hex) = input.strip_prefix(BALANCE_PREFIX) {
            let addr = checksummed_hex::decode(hex)
                .map_err(|error| FromStrError::Balance(error.to_string()))?;
            let uref_addr = URefAddr::try_from(addr.as_ref())
                .map_err(|error| FromStrError::Balance(error.to_string()))?;
            return Ok(Key::Balance(uref_addr));
        }

        if let Some(hex) = input.strip_prefix(BID_PREFIX) {
            let hash = checksummed_hex::decode(hex)
                .map_err(|error| FromStrError::Bid(error.to_string()))?;
            let account_hash = <[u8; ACCOUNT_HASH_LENGTH]>::try_from(hash.as_ref())
                .map_err(|error| FromStrError::Bid(error.to_string()))?;
            return Ok(Key::Bid(AccountHash::new(account_hash)));
        }

        if let Some(hex) = input.strip_prefix(WITHDRAW_PREFIX) {
            let hash = checksummed_hex::decode(hex)
                .map_err(|error| FromStrError::Withdraw(error.to_string()))?;
            let account_hash = <[u8; ACCOUNT_HASH_LENGTH]>::try_from(hash.as_ref())
                .map_err(|error| FromStrError::Withdraw(error.to_string()))?;
            return Ok(Key::Withdraw(AccountHash::new(account_hash)));
        }

        if let Some(hex) = input.strip_prefix(UNBOND_PREFIX) {
            let hash = checksummed_hex::decode(hex)
                .map_err(|error| FromStrError::Unbond(error.to_string()))?;
            let account_hash = <[u8; ACCOUNT_HASH_LENGTH]>::try_from(hash.as_ref())
                .map_err(|error| FromStrError::Unbond(error.to_string()))?;
            return Ok(Key::Unbond(AccountHash::new(account_hash)));
        }

        if let Some(dictionary_addr) = input.strip_prefix(DICTIONARY_PREFIX) {
            let dictionary_addr_bytes = checksummed_hex::decode(dictionary_addr)
                .map_err(|error| FromStrError::Dictionary(error.to_string()))?;
            let addr = DictionaryAddr::try_from(dictionary_addr_bytes.as_ref())
                .map_err(|error| FromStrError::Dictionary(error.to_string()))?;
            return Ok(Key::Dictionary(addr));
        }

        if let Some(registry_padding) = input.strip_prefix(SYSTEM_CONTRACT_REGISTRY_PREFIX) {
            let padded_bytes = checksummed_hex::decode(registry_padding)
                .map_err(|error| FromStrError::SystemContractRegistry(error.to_string()))?;
            let _padding: [u8; 32] = TryFrom::try_from(padded_bytes.as_ref()).map_err(|_| {
                FromStrError::SystemContractRegistry(
                    "Failed to deserialize system registry key".to_string(),
                )
            })?;
            return Ok(Key::SystemContractRegistry);
        }

        if let Some(registry_padding) = input.strip_prefix(CHAINSPEC_REGISTRY_PREFIX) {
            let padded_bytes = checksummed_hex::decode(registry_padding)
                .map_err(|error| FromStrError::ChainspecRegistry(error.to_string()))?;
            let _padding: [u8; 32] = TryFrom::try_from(padded_bytes.as_ref()).map_err(|_| {
                FromStrError::ChainspecRegistry(
                    "Failed to deserialize chainspec registry key".to_string(),
                )
            })?;
            return Ok(Key::ChainspecRegistry);
        }

        if let Some(block_height_str) = input.strip_prefix(BLOCK_EFFECTS_ROOT_HASH_PREFIX) {
            let block_height = u64::from_str(block_height_str)
                .map_err(|error| FromStrError::BlockEffectsRootHash(error.to_string()))?;
            return Ok(Key::BlockEffectsRootHash { block_height });
        }

        Err(FromStrError::UnknownPrefix)
    }

    /// Returns the inner bytes of `self` if `self` is of type [`Key::Account`], otherwise returns
    /// `None`.
    pub fn into_account(self) -> Option<AccountHash> {
        match self {
            Key::Account(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Returns the inner bytes of `self` if `self` is of type [`Key::Hash`], otherwise returns
    /// `None`.
    pub fn into_hash(self) -> Option<HashAddr> {
        match self {
            Key::Hash(hash) => Some(hash),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`URef`] if `self` is of type [`Key::URef`], otherwise
    /// returns `None`.
    pub fn as_uref(&self) -> Option<&URef> {
        match self {
            Key::URef(uref) => Some(uref),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`URef`] if `self` is of type [`Key::URef`], otherwise
    /// returns `None`.
    pub fn as_uref_mut(&mut self) -> Option<&mut URef> {
        match self {
            Key::URef(uref) => Some(uref),
            _ => None,
        }
    }

    /// Returns a reference to the inner `URefAddr` if `self` is of type [`Key::Balance`],
    /// otherwise returns `None`.
    pub fn as_balance(&self) -> Option<&URefAddr> {
        if let Self::Balance(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Returns the inner [`URef`] if `self` is of type [`Key::URef`], otherwise returns `None`.
    pub fn into_uref(self) -> Option<URef> {
        match self {
            Key::URef(uref) => Some(uref),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`DictionaryAddr`] if `self` is of type
    /// [`Key::Dictionary`], otherwise returns `None`.
    pub fn as_dictionary(&self) -> Option<&DictionaryAddr> {
        match self {
            Key::Dictionary(v) => Some(v),
            _ => None,
        }
    }

    /// Casts a [`Key::URef`] to a [`Key::Hash`]
    pub fn uref_to_hash(&self) -> Option<Key> {
        let uref = self.as_uref()?;
        let addr = uref.addr();
        Some(Key::Hash(addr))
    }

    /// Casts a [`Key::Withdraw`] to a [`Key::Unbond`]
    pub fn withdraw_to_unbond(&self) -> Option<Key> {
        if let Key::Withdraw(account_hash) = self {
            return Some(Key::Unbond(*account_hash));
        }
        None
    }

    /// Creates a new [`Key::Dictionary`] variant based on a `seed_uref` and a `dictionary_item_key`
    /// bytes.
    pub fn dictionary(seed_uref: URef, dictionary_item_key: &[u8]) -> Key {
        // NOTE: Expect below is safe because the length passed is supported.
        let mut hasher = VarBlake2b::new(BLAKE2B_DIGEST_LENGTH).expect("should create hasher");
        hasher.update(seed_uref.addr().as_ref());
        hasher.update(dictionary_item_key);
        // NOTE: Assumed safe as size of `HashAddr` equals to the output provided by hasher.
        let mut addr = HashAddr::default();
        hasher.finalize_variable(|hash| addr.clone_from_slice(hash));
        Key::Dictionary(addr)
    }

    /// Returns true if the key is of type [`Key::Dictionary`].
    pub fn is_dictionary_key(&self) -> bool {
        if let Key::Dictionary(_) = self {
            return true;
        }
        false
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Key::Account(account_hash) => write!(f, "Key::Account({})", account_hash),
            Key::Hash(addr) => write!(f, "Key::Hash({})", base16::encode_lower(&addr)),
            Key::URef(uref) => write!(f, "Key::{}", uref), /* Display impl for URef will append */
            Key::Transfer(transfer_addr) => write!(f, "Key::Transfer({})", transfer_addr),
            Key::DeployInfo(addr) => write!(
                f,
                "Key::DeployInfo({})",
                base16::encode_lower(addr.as_bytes())
            ),
            Key::EraInfo(era_id) => write!(f, "Key::EraInfo({})", era_id),
            Key::Balance(uref_addr) => {
                write!(f, "Key::Balance({})", base16::encode_lower(uref_addr))
            }
            Key::Bid(account_hash) => write!(f, "Key::Bid({})", account_hash),
            Key::Withdraw(account_hash) => write!(f, "Key::Withdraw({})", account_hash),
            Key::Unbond(account_hash) => write!(f, "Key::Unbond({})", account_hash),
            Key::Dictionary(addr) => {
                write!(f, "Key::Dictionary({})", base16::encode_lower(addr))
            }
            Key::SystemContractRegistry => write!(
                f,
                "Key::SystemContractRegistry({})",
                base16::encode_lower(&SYSTEM_CONTRACT_REGISTRY_KEY_BYTES)
            ),
            Key::ChainspecRegistry => write!(
                f,
                "Key::ChainspecRegistry({})",
                base16::encode_lower(&CHAINSPEC_REGISTRY_KEY_BYTES)
            ),
            Key::BlockEffectsRootHash { block_height } => {
                write!(f, "Key::BlockEffectsRootHash({})", block_height)
            }
            Key::DeployApprovalsRootHash { block_height } => {
                write!(f, "Key::DeployApprovalsRootHash({})", block_height)
            }
        }
    }
}

impl Debug for Key {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl Tagged<KeyTag> for Key {
    fn tag(&self) -> KeyTag {
        match self {
            Key::Account(_) => KeyTag::Account,
            Key::Hash(_) => KeyTag::Hash,
            Key::URef(_) => KeyTag::URef,
            Key::Transfer(_) => KeyTag::Transfer,
            Key::DeployInfo(_) => KeyTag::DeployInfo,
            Key::EraInfo(_) => KeyTag::EraInfo,
            Key::Balance(_) => KeyTag::Balance,
            Key::Bid(_) => KeyTag::Bid,
            Key::Withdraw(_) => KeyTag::Withdraw,
            Key::Unbond(_) => KeyTag::Unbond,
            Key::Dictionary(_) => KeyTag::Dictionary,
            Key::SystemContractRegistry => KeyTag::SystemContractRegistry,
            Key::ChainspecRegistry => KeyTag::ChainspecRegistry,
            Key::BlockEffectsRootHash { .. } => KeyTag::BlockEffectsRootHash,
            Key::DeployApprovalsRootHash { .. } => KeyTag::DeployApprovalsRootHash,
        }
    }
}

impl Tagged<u8> for Key {
    fn tag(&self) -> u8 {
        let key_tag: KeyTag = self.tag();
        key_tag as u8
    }
}

impl From<URef> for Key {
    fn from(uref: URef) -> Key {
        Key::URef(uref)
    }
}

impl From<AccountHash> for Key {
    fn from(account_hash: AccountHash) -> Key {
        Key::Account(account_hash)
    }
}

impl From<TransferAddr> for Key {
    fn from(transfer_addr: TransferAddr) -> Key {
        Key::Transfer(transfer_addr)
    }
}

impl From<ContractHash> for Key {
    fn from(contract_hash: ContractHash) -> Key {
        Key::Hash(contract_hash.value())
    }
}

impl From<ContractWasmHash> for Key {
    fn from(wasm_hash: ContractWasmHash) -> Key {
        Key::Hash(wasm_hash.value())
    }
}

impl From<ContractPackageHash> for Key {
    fn from(package_hash: ContractPackageHash) -> Key {
        Key::Hash(package_hash.value())
    }
}

impl ToBytes for Key {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut result = bytesrepr::unchecked_allocate_buffer(self);
        result.push(self.tag());
        match self {
            Key::Account(account_hash) => {
                result.append(&mut account_hash.to_bytes()?);
            }
            Key::Hash(hash) => {
                result.append(&mut hash.to_bytes()?);
            }
            Key::URef(uref) => {
                result.append(&mut uref.to_bytes()?);
            }
            Key::Transfer(addr) => {
                result.append(&mut addr.to_bytes()?);
            }
            Key::DeployInfo(addr) => {
                result.append(&mut addr.to_bytes()?);
            }
            Key::EraInfo(era_id) => {
                result.append(&mut era_id.to_bytes()?);
            }
            Key::Balance(uref_addr) => {
                result.append(&mut uref_addr.to_bytes()?);
            }
            Key::Bid(account_hash) => {
                result.append(&mut account_hash.to_bytes()?);
            }
            Key::Withdraw(account_hash) => {
                result.append(&mut account_hash.to_bytes()?);
            }
            Key::Unbond(account_hash) => {
                result.append(&mut account_hash.to_bytes()?);
            }
            Key::Dictionary(addr) => {
                result.append(&mut addr.to_bytes()?);
            }
            Key::SystemContractRegistry => {
                result.append(&mut SYSTEM_CONTRACT_REGISTRY_KEY_BYTES.to_bytes()?)
            }
            Key::ChainspecRegistry => result.append(&mut CHAINSPEC_REGISTRY_KEY_BYTES.to_bytes()?),
            Key::BlockEffectsRootHash { block_height } => {
                result.append(&mut block_height.to_bytes()?)
            }
            Key::DeployApprovalsRootHash { block_height } => {
                result.append(&mut block_height.to_bytes()?)
            }
        }
        Ok(result)
    }

    fn serialized_length(&self) -> usize {
        match self {
            Key::Account(account_hash) => {
                KEY_ID_SERIALIZED_LENGTH + account_hash.serialized_length()
            }
            Key::Hash(_) => KEY_HASH_SERIALIZED_LENGTH,
            Key::URef(_) => KEY_UREF_SERIALIZED_LENGTH,
            Key::Transfer(_) => KEY_TRANSFER_SERIALIZED_LENGTH,
            Key::DeployInfo(_) => KEY_DEPLOY_INFO_SERIALIZED_LENGTH,
            Key::EraInfo(_) => KEY_ERA_INFO_SERIALIZED_LENGTH,
            Key::Balance(_) => KEY_BALANCE_SERIALIZED_LENGTH,
            Key::Bid(_) => KEY_BID_SERIALIZED_LENGTH,
            Key::Withdraw(_) => KEY_WITHDRAW_SERIALIZED_LENGTH,
            Key::Unbond(_) => KEY_UNBOND_SERIALIZED_LENGTH,
            Key::Dictionary(_) => KEY_DICTIONARY_SERIALIZED_LENGTH,
            Key::SystemContractRegistry => KEY_SYSTEM_CONTRACT_REGISTRY_SERIALIZED_LENGTH,
            Key::ChainspecRegistry => KEY_CHAINSPEC_REGISTRY_SERIALIZED_LENGTH,
            Key::BlockEffectsRootHash { .. } => KEY_BLOCK_EFFECTS_ROOT_HASH_LENGTH,
            Key::DeployApprovalsRootHash { .. } => KEY_DEPLOY_APPROVALS_ROOT_HASH_SERIALIZED_LENGTH,
        }
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), Error> {
        writer.push(self.tag());
        match self {
            Key::Account(account_hash) => account_hash.write_bytes(writer),
            Key::Hash(hash) => hash.write_bytes(writer),
            Key::URef(uref) => uref.write_bytes(writer),
            Key::Transfer(addr) => addr.write_bytes(writer),
            Key::DeployInfo(deploy_hash) => deploy_hash.write_bytes(writer),
            Key::EraInfo(era_id) => era_id.write_bytes(writer),
            Key::Balance(uref_addr) => uref_addr.write_bytes(writer),
            Key::Bid(account_hash) => account_hash.write_bytes(writer),
            Key::Withdraw(account_hash) => account_hash.write_bytes(writer),
            Key::Unbond(account_hash) => account_hash.write_bytes(writer),
            Key::Dictionary(addr) => addr.write_bytes(writer),
            Key::SystemContractRegistry => SYSTEM_CONTRACT_REGISTRY_KEY_BYTES.write_bytes(writer),
            Key::ChainspecRegistry => CHAINSPEC_REGISTRY_KEY_BYTES.write_bytes(writer),
            Key::BlockEffectsRootHash { block_height } => block_height.write_bytes(writer),
            Key::DeployApprovalsRootHash { block_height } => block_height.write_bytes(writer),
        }
    }
}

impl FromBytes for Key {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), Error> {
        let (tag, remainder) = u8::from_bytes(bytes)?;
        match tag {
            tag if tag == KeyTag::Account as u8 => {
                let (account_hash, rem) = AccountHash::from_bytes(remainder)?;
                Ok((Key::Account(account_hash), rem))
            }
            tag if tag == KeyTag::Hash as u8 => {
                let (hash, rem) = HashAddr::from_bytes(remainder)?;
                Ok((Key::Hash(hash), rem))
            }
            tag if tag == KeyTag::URef as u8 => {
                let (uref, rem) = URef::from_bytes(remainder)?;
                Ok((Key::URef(uref), rem))
            }
            tag if tag == KeyTag::Transfer as u8 => {
                let (transfer_addr, rem) = TransferAddr::from_bytes(remainder)?;
                Ok((Key::Transfer(transfer_addr), rem))
            }
            tag if tag == KeyTag::DeployInfo as u8 => {
                let (deploy_hash, rem) = DeployHash::from_bytes(remainder)?;
                Ok((Key::DeployInfo(deploy_hash), rem))
            }
            tag if tag == KeyTag::EraInfo as u8 => {
                let (era_id, rem) = EraId::from_bytes(remainder)?;
                Ok((Key::EraInfo(era_id), rem))
            }
            tag if tag == KeyTag::Balance as u8 => {
                let (uref_addr, rem) = URefAddr::from_bytes(remainder)?;
                Ok((Key::Balance(uref_addr), rem))
            }
            tag if tag == KeyTag::Bid as u8 => {
                let (account_hash, rem) = AccountHash::from_bytes(remainder)?;
                Ok((Key::Bid(account_hash), rem))
            }
            tag if tag == KeyTag::Withdraw as u8 => {
                let (account_hash, rem) = AccountHash::from_bytes(remainder)?;
                Ok((Key::Withdraw(account_hash), rem))
            }
            tag if tag == KeyTag::Unbond as u8 => {
                let (account_hash, rem) = AccountHash::from_bytes(remainder)?;
                Ok((Key::Unbond(account_hash), rem))
            }
            tag if tag == KeyTag::Dictionary as u8 => {
                let (addr, rem) = DictionaryAddr::from_bytes(remainder)?;
                Ok((Key::Dictionary(addr), rem))
            }
            tag if tag == KeyTag::SystemContractRegistry as u8 => {
                let (_, rem) = <[u8; 32]>::from_bytes(remainder)?;
                Ok((Key::SystemContractRegistry, rem))
            }
            tag if tag == KeyTag::ChainspecRegistry as u8 => {
                let (_, rem) = <[u8; 32]>::from_bytes(remainder)?;
                Ok((Key::ChainspecRegistry, rem))
            }
            tag if tag == KeyTag::BlockEffectsRootHash as u8 => {
                let (block_height, rem) = u64::from_bytes(remainder)?;
                Ok((Key::BlockEffectsRootHash { block_height }, rem))
            }
            tag if tag == KeyTag::DeployApprovalsRootHash as u8 => {
                let (block_height, rem) = u64::from_bytes(remainder)?;
                Ok((Key::DeployApprovalsRootHash { block_height }, rem))
            }
            _ => Err(Error::Formatting),
        }
    }
}

#[allow(dead_code)]
fn please_add_to_distribution_impl(key: Key) {
    // If you've been forced to come here, you likely need to add your variant to the
    // `Distribution` impl for `Key`.
    match key {
        Key::Account(_) => unimplemented!(),
        Key::Hash(_) => unimplemented!(),
        Key::URef(_) => unimplemented!(),
        Key::Transfer(_) => unimplemented!(),
        Key::DeployInfo(_) => unimplemented!(),
        Key::EraInfo(_) => unimplemented!(),
        Key::Balance(_) => unimplemented!(),
        Key::Bid(_) => unimplemented!(),
        Key::Withdraw(_) => unimplemented!(),
        Key::Dictionary(_) => unimplemented!(),
        Key::SystemContractRegistry => unimplemented!(),
        Key::Unbond(_) => unimplemented!(),
        Key::ChainspecRegistry => unimplemented!(),
        Key::BlockEffectsRootHash { .. } => unimplemented!(),
        Key::DeployApprovalsRootHash { .. } => unimplemented!(),
    }
}

impl Distribution<Key> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Key {
        match rng.gen_range(0..=14) {
            0 => Key::Account(rng.gen()),
            1 => Key::Hash(rng.gen()),
            2 => Key::URef(rng.gen()),
            3 => Key::Transfer(rng.gen()),
            4 => Key::DeployInfo(rng.gen()),
            5 => Key::EraInfo(rng.gen()),
            6 => Key::Balance(rng.gen()),
            7 => Key::Bid(rng.gen()),
            8 => Key::Withdraw(rng.gen()),
            9 => Key::Dictionary(rng.gen()),
            10 => Key::SystemContractRegistry,
            11 => Key::Unbond(rng.gen()),
            12 => Key::ChainspecRegistry,
            13 => Key::BlockEffectsRootHash {
                block_height: rng.gen(),
            },
            14 => Key::DeployApprovalsRootHash {
                block_height: rng.gen(),
            },
            _ => unreachable!(),
        }
    }
}

mod serde_helpers {
    use super::*;

    #[derive(Serialize, Deserialize)]
    pub(super) enum HumanReadable {
        Account(String),
        Hash(String),
        URef(String),
        Transfer(String),
        DeployInfo(String),
        EraInfo(String),
        Balance(String),
        Bid(String),
        Withdraw(String),
        Dictionary(String),
        SystemContractRegistry(String),
        Unbond(String),
        ChainspecRegistry(String),
        BlockEffectsRootHash(String),
        DeployApprovalsRootHash(String),
    }

    impl From<&Key> for HumanReadable {
        fn from(key: &Key) -> Self {
            let formatted_string = key.to_formatted_string();
            match key {
                Key::Account(_) => HumanReadable::Account(formatted_string),
                Key::Hash(_) => HumanReadable::Hash(formatted_string),
                Key::URef(_) => HumanReadable::URef(formatted_string),
                Key::Transfer(_) => HumanReadable::Transfer(formatted_string),
                Key::DeployInfo(_) => HumanReadable::DeployInfo(formatted_string),
                Key::EraInfo(_) => HumanReadable::EraInfo(formatted_string),
                Key::Balance(_) => HumanReadable::Balance(formatted_string),
                Key::Bid(_) => HumanReadable::Bid(formatted_string),
                Key::Withdraw(_) => HumanReadable::Withdraw(formatted_string),
                Key::Unbond(_) => HumanReadable::Unbond(formatted_string),
                Key::Dictionary(_) => HumanReadable::Dictionary(formatted_string),
                Key::SystemContractRegistry => {
                    HumanReadable::SystemContractRegistry(formatted_string)
                }
                Key::ChainspecRegistry => HumanReadable::ChainspecRegistry(formatted_string),
                Key::BlockEffectsRootHash { .. } => {
                    HumanReadable::BlockEffectsRootHash(formatted_string)
                }
                Key::DeployApprovalsRootHash { .. } => {
                    HumanReadable::DeployApprovalsRootHash(formatted_string)
                }
            }
        }
    }

    impl TryFrom<HumanReadable> for Key {
        type Error = FromStrError;

        fn try_from(helper: HumanReadable) -> Result<Self, Self::Error> {
            match helper {
                HumanReadable::Account(formatted_string)
                | HumanReadable::Hash(formatted_string)
                | HumanReadable::URef(formatted_string)
                | HumanReadable::Transfer(formatted_string)
                | HumanReadable::DeployInfo(formatted_string)
                | HumanReadable::EraInfo(formatted_string)
                | HumanReadable::Balance(formatted_string)
                | HumanReadable::Bid(formatted_string)
                | HumanReadable::Withdraw(formatted_string)
                | HumanReadable::Unbond(formatted_string)
                | HumanReadable::Dictionary(formatted_string)
                | HumanReadable::SystemContractRegistry(formatted_string)
                | HumanReadable::ChainspecRegistry(formatted_string)
                | HumanReadable::BlockEffectsRootHash(formatted_string)
                | HumanReadable::DeployApprovalsRootHash(formatted_string) => {
                    Key::from_formatted_str(&formatted_string)
                }
            }
        }
    }

    #[derive(Serialize)]
    pub(super) enum BinarySerHelper<'a> {
        Account(&'a AccountHash),
        Hash(&'a HashAddr),
        URef(&'a URef),
        Transfer(&'a TransferAddr),
        DeployInfo(&'a DeployHash),
        EraInfo(&'a EraId),
        Balance(&'a URefAddr),
        Bid(&'a AccountHash),
        Withdraw(&'a AccountHash),
        Dictionary(&'a HashAddr),
        SystemContractRegistry,
        Unbond(&'a AccountHash),
        ChainspecRegistry,
        BlockEffectsRootHash { block_height: u64 },
        DeployApprovalsRootHash { block_height: u64 },
    }

    impl<'a> From<&'a Key> for BinarySerHelper<'a> {
        fn from(key: &'a Key) -> Self {
            match key {
                Key::Account(account_hash) => BinarySerHelper::Account(account_hash),
                Key::Hash(hash_addr) => BinarySerHelper::Hash(hash_addr),
                Key::URef(uref) => BinarySerHelper::URef(uref),
                Key::Transfer(transfer_addr) => BinarySerHelper::Transfer(transfer_addr),
                Key::DeployInfo(deploy_hash) => BinarySerHelper::DeployInfo(deploy_hash),
                Key::EraInfo(era_id) => BinarySerHelper::EraInfo(era_id),
                Key::Balance(uref_addr) => BinarySerHelper::Balance(uref_addr),
                Key::Bid(account_hash) => BinarySerHelper::Bid(account_hash),
                Key::Withdraw(account_hash) => BinarySerHelper::Withdraw(account_hash),
                Key::Unbond(account_hash) => BinarySerHelper::Unbond(account_hash),
                Key::Dictionary(addr) => BinarySerHelper::Dictionary(addr),
                Key::SystemContractRegistry => BinarySerHelper::SystemContractRegistry,
                Key::ChainspecRegistry => BinarySerHelper::ChainspecRegistry,
                Key::BlockEffectsRootHash { block_height } => {
                    BinarySerHelper::BlockEffectsRootHash {
                        block_height: *block_height,
                    }
                }
                Key::DeployApprovalsRootHash { block_height } => {
                    BinarySerHelper::DeployApprovalsRootHash {
                        block_height: *block_height,
                    }
                }
            }
        }
    }

    #[derive(Deserialize)]
    pub(super) enum BinaryDeserHelper {
        Account(AccountHash),
        Hash(HashAddr),
        URef(URef),
        Transfer(TransferAddr),
        DeployInfo(DeployHash),
        EraInfo(EraId),
        Balance(URefAddr),
        Bid(AccountHash),
        Withdraw(AccountHash),
        Dictionary(DictionaryAddr),
        SystemContractRegistry,
        Unbond(AccountHash),
        ChainspecRegistry,
        BlockEffectsRootHash { block_height: u64 },
        DeployApprovalsRootHash { block_height: u64 },
    }

    impl From<BinaryDeserHelper> for Key {
        fn from(helper: BinaryDeserHelper) -> Self {
            match helper {
                BinaryDeserHelper::Account(account_hash) => Key::Account(account_hash),
                BinaryDeserHelper::Hash(hash_addr) => Key::Hash(hash_addr),
                BinaryDeserHelper::URef(uref) => Key::URef(uref),
                BinaryDeserHelper::Transfer(transfer_addr) => Key::Transfer(transfer_addr),
                BinaryDeserHelper::DeployInfo(deploy_hash) => Key::DeployInfo(deploy_hash),
                BinaryDeserHelper::EraInfo(era_id) => Key::EraInfo(era_id),
                BinaryDeserHelper::Balance(uref_addr) => Key::Balance(uref_addr),
                BinaryDeserHelper::Bid(account_hash) => Key::Bid(account_hash),
                BinaryDeserHelper::Withdraw(account_hash) => Key::Withdraw(account_hash),
                BinaryDeserHelper::Unbond(account_hash) => Key::Unbond(account_hash),
                BinaryDeserHelper::Dictionary(addr) => Key::Dictionary(addr),
                BinaryDeserHelper::SystemContractRegistry => Key::SystemContractRegistry,
                BinaryDeserHelper::ChainspecRegistry => Key::ChainspecRegistry,
                BinaryDeserHelper::BlockEffectsRootHash { block_height } => {
                    Key::BlockEffectsRootHash { block_height }
                }
                BinaryDeserHelper::DeployApprovalsRootHash { block_height } => {
                    Key::DeployApprovalsRootHash { block_height }
                }
            }
        }
    }
}

impl Serialize for Key {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serde_helpers::HumanReadable::from(self).serialize(serializer)
        } else {
            serde_helpers::BinarySerHelper::from(self).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            let human_readable = serde_helpers::HumanReadable::deserialize(deserializer)?;
            Key::try_from(human_readable).map_err(SerdeError::custom)
        } else {
            let binary_helper = serde_helpers::BinaryDeserHelper::deserialize(deserializer)?;
            Ok(Key::from(binary_helper))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::string::ToString;

    use super::*;
    use crate::{
        account::ACCOUNT_HASH_FORMATTED_STRING_PREFIX,
        bytesrepr::{Error, FromBytes},
        transfer::TRANSFER_ADDR_FORMATTED_STRING_PREFIX,
        uref::UREF_FORMATTED_STRING_PREFIX,
        AccessRights, URef,
    };

    const BLOCK_HEIGHT: u64 = 245125;

    const ACCOUNT_KEY: Key = Key::Account(AccountHash::new([42; 32]));
    const HASH_KEY: Key = Key::Hash([42; 32]);
    const UREF_KEY: Key = Key::URef(URef::new([42; 32], AccessRights::READ));
    const TRANSFER_KEY: Key = Key::Transfer(TransferAddr::new([42; 32]));
    const DEPLOY_INFO_KEY: Key = Key::DeployInfo(DeployHash::new([42; 32]));
    const ERA_INFO_KEY: Key = Key::EraInfo(EraId::new(42));
    const BALANCE_KEY: Key = Key::Balance([42; 32]);
    const BID_KEY: Key = Key::Bid(AccountHash::new([42; 32]));
    const WITHDRAW_KEY: Key = Key::Withdraw(AccountHash::new([42; 32]));
    const DICTIONARY_KEY: Key = Key::Dictionary([42; 32]);
    const SYSTEM_CONTRACT_REGISTRY_KEY: Key = Key::SystemContractRegistry;
    const CHAINSPEC_REGISTRY_KEY: Key = Key::ChainspecRegistry;
    const UNBOND_KEY: Key = Key::Unbond(AccountHash::new([42; 32]));
    const BLOCK_EFFECTS_ROOT_HASH_KEY: Key = Key::BlockEffectsRootHash {
        block_height: BLOCK_HEIGHT,
    };
    const DEPLOY_APPROVALS_ROOT_HASH_KEY: Key = Key::DeployApprovalsRootHash {
        block_height: BLOCK_HEIGHT,
    };
    const KEYS: [Key; 15] = [
        ACCOUNT_KEY,
        HASH_KEY,
        UREF_KEY,
        TRANSFER_KEY,
        DEPLOY_INFO_KEY,
        ERA_INFO_KEY,
        BALANCE_KEY,
        BID_KEY,
        WITHDRAW_KEY,
        DICTIONARY_KEY,
        SYSTEM_CONTRACT_REGISTRY_KEY,
        CHAINSPEC_REGISTRY_KEY,
        UNBOND_KEY,
        BLOCK_EFFECTS_ROOT_HASH_KEY,
        DEPLOY_APPROVALS_ROOT_HASH_KEY,
    ];
    const HEX_STRING: &str = "2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a";

    fn test_readable(right: AccessRights, is_true: bool) {
        assert_eq!(right.is_readable(), is_true)
    }

    #[test]
    fn test_is_readable() {
        test_readable(AccessRights::READ, true);
        test_readable(AccessRights::READ_ADD, true);
        test_readable(AccessRights::READ_WRITE, true);
        test_readable(AccessRights::READ_ADD_WRITE, true);
        test_readable(AccessRights::ADD, false);
        test_readable(AccessRights::ADD_WRITE, false);
        test_readable(AccessRights::WRITE, false);
    }

    fn test_writable(right: AccessRights, is_true: bool) {
        assert_eq!(right.is_writeable(), is_true)
    }

    #[test]
    fn test_is_writable() {
        test_writable(AccessRights::WRITE, true);
        test_writable(AccessRights::READ_WRITE, true);
        test_writable(AccessRights::ADD_WRITE, true);
        test_writable(AccessRights::READ, false);
        test_writable(AccessRights::ADD, false);
        test_writable(AccessRights::READ_ADD, false);
        test_writable(AccessRights::READ_ADD_WRITE, true);
    }

    fn test_addable(right: AccessRights, is_true: bool) {
        assert_eq!(right.is_addable(), is_true)
    }

    #[test]
    fn test_is_addable() {
        test_addable(AccessRights::ADD, true);
        test_addable(AccessRights::READ_ADD, true);
        test_addable(AccessRights::READ_WRITE, false);
        test_addable(AccessRights::ADD_WRITE, true);
        test_addable(AccessRights::READ, false);
        test_addable(AccessRights::WRITE, false);
        test_addable(AccessRights::READ_ADD_WRITE, true);
    }

    #[test]
    fn should_display_key() {
        assert_eq!(
            format!("{}", ACCOUNT_KEY),
            format!("Key::Account({})", HEX_STRING)
        );
        assert_eq!(
            format!("{}", HASH_KEY),
            format!("Key::Hash({})", HEX_STRING)
        );
        assert_eq!(
            format!("{}", UREF_KEY),
            format!("Key::URef({}, READ)", HEX_STRING)
        );
        assert_eq!(
            format!("{}", TRANSFER_KEY),
            format!("Key::Transfer({})", HEX_STRING)
        );
        assert_eq!(
            format!("{}", DEPLOY_INFO_KEY),
            format!("Key::DeployInfo({})", HEX_STRING)
        );
        assert_eq!(
            format!("{}", ERA_INFO_KEY),
            "Key::EraInfo(era 42)".to_string()
        );
        assert_eq!(
            format!("{}", BALANCE_KEY),
            format!("Key::Balance({})", HEX_STRING)
        );
        assert_eq!(format!("{}", BID_KEY), format!("Key::Bid({})", HEX_STRING));
        assert_eq!(
            format!("{}", WITHDRAW_KEY),
            format!("Key::Withdraw({})", HEX_STRING)
        );
        assert_eq!(
            format!("{}", UNBOND_KEY),
            format!("Key::Unbond({})", HEX_STRING)
        );
        assert_eq!(
            format!("{}", DICTIONARY_KEY),
            format!("Key::Dictionary({})", HEX_STRING)
        );
        assert_eq!(
            format!("{}", SYSTEM_CONTRACT_REGISTRY_KEY),
            format!(
                "Key::SystemContractRegistry({})",
                base16::encode_lower(&SYSTEM_CONTRACT_REGISTRY_KEY_BYTES)
            )
        );
        assert_eq!(
            format!("{}", CHAINSPEC_REGISTRY_KEY),
            format!(
                "Key::ChainspecRegistry({})",
                base16::encode_lower(&CHAINSPEC_REGISTRY_KEY_BYTES)
            )
        );
        assert_eq!(
            format!("{}", BLOCK_EFFECTS_ROOT_HASH_KEY),
            format!("Key::BlockEffectsRootHash({})", BLOCK_HEIGHT,)
        );
        assert_eq!(
            format!("{}", DEPLOY_APPROVALS_ROOT_HASH_KEY),
            format!("Key::DeployApprovalsRootHash({})", BLOCK_HEIGHT,)
        );
    }

    #[test]
    fn abuse_vec_key() {
        // Prefix is 2^32-1 = shouldn't allocate that much
        let bytes: Vec<u8> = vec![255, 255, 255, 255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let res: Result<(Vec<Key>, &[u8]), _> = FromBytes::from_bytes(&bytes);
        #[cfg(target_os = "linux")]
        assert_eq!(res.expect_err("should fail"), Error::OutOfMemory);
        #[cfg(target_os = "macos")]
        assert_eq!(res.expect_err("should fail"), Error::EarlyEndOfStream);
    }

    #[test]
    fn check_key_account_getters() {
        let account = [42; 32];
        let account_hash = AccountHash::new(account);
        let key1 = Key::Account(account_hash);
        assert_eq!(key1.into_account(), Some(account_hash));
        assert!(key1.into_hash().is_none());
        assert!(key1.as_uref().is_none());
    }

    #[test]
    fn check_key_hash_getters() {
        let hash = [42; KEY_HASH_LENGTH];
        let key1 = Key::Hash(hash);
        assert!(key1.into_account().is_none());
        assert_eq!(key1.into_hash(), Some(hash));
        assert!(key1.as_uref().is_none());
    }

    #[test]
    fn check_key_uref_getters() {
        let uref = URef::new([42; 32], AccessRights::READ_ADD_WRITE);
        let key1 = Key::URef(uref);
        assert!(key1.into_account().is_none());
        assert!(key1.into_hash().is_none());
        assert_eq!(key1.as_uref(), Some(&uref));
    }

    #[test]
    fn key_max_serialized_length() {
        let mut got_max = false;
        for key in &KEYS {
            assert!(key.serialized_length() <= Key::max_serialized_length());
            if key.serialized_length() == Key::max_serialized_length() {
                got_max = true;
            }
        }
        assert!(
            got_max,
            "None of the Key variants has a serialized_length equal to \
            Key::max_serialized_length(), so Key::max_serialized_length() should be reduced"
        );
    }

    #[test]
    fn should_parse_key_from_str() {
        for key in &KEYS {
            let string = key.to_formatted_string();
            let parsed_key = Key::from_formatted_str(&string).unwrap();
            assert_eq!(*key, parsed_key);
        }
    }

    #[test]
    fn should_fail_to_parse_key_from_str() {
        assert!(
            Key::from_formatted_str(ACCOUNT_HASH_FORMATTED_STRING_PREFIX)
                .unwrap_err()
                .to_string()
                .starts_with("account-key from string error: ")
        );
        assert!(Key::from_formatted_str(HASH_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("hash-key from string error: "));
        assert!(Key::from_formatted_str(UREF_FORMATTED_STRING_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("uref-key from string error: "));
        assert!(
            Key::from_formatted_str(TRANSFER_ADDR_FORMATTED_STRING_PREFIX)
                .unwrap_err()
                .to_string()
                .starts_with("transfer-key from string error: ")
        );
        assert!(Key::from_formatted_str(DEPLOY_INFO_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("deploy-info-key from string error: "));
        assert!(Key::from_formatted_str(ERA_INFO_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("era-info-key from string error: "));
        assert!(Key::from_formatted_str(BALANCE_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("balance-key from string error: "));
        assert!(Key::from_formatted_str(BID_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("bid-key from string error: "));
        assert!(Key::from_formatted_str(WITHDRAW_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("withdraw-key from string error: "));
        assert!(Key::from_formatted_str(UNBOND_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("unbond-key from string error: "));
        assert!(Key::from_formatted_str(DICTIONARY_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("dictionary-key from string error: "));
        assert!(Key::from_formatted_str(SYSTEM_CONTRACT_REGISTRY_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("system-contract-registry-key from string error: "));
        assert!(Key::from_formatted_str(CHAINSPEC_REGISTRY_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("chainspec-registry-key from string error: "));
        assert!(Key::from_formatted_str(BLOCK_EFFECTS_ROOT_HASH_PREFIX)
            .unwrap_err()
            .to_string()
            .starts_with("block-effects-root-hash-key from string error: "));

        let invalid_prefix = "a-0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(
            Key::from_formatted_str(invalid_prefix)
                .unwrap_err()
                .to_string(),
            "unknown prefix for key"
        );

        let missing_hyphen_prefix =
            "hash0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(
            Key::from_formatted_str(missing_hyphen_prefix)
                .unwrap_err()
                .to_string(),
            "unknown prefix for key"
        );

        let no_prefix = "0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(
            Key::from_formatted_str(no_prefix).unwrap_err().to_string(),
            "unknown prefix for key"
        );
    }

    #[test]
    fn key_to_json() {
        let expected_json = [
            format!(r#"{{"Account":"account-hash-{}"}}"#, HEX_STRING),
            format!(r#"{{"Hash":"hash-{}"}}"#, HEX_STRING),
            format!(r#"{{"URef":"uref-{}-001"}}"#, HEX_STRING),
            format!(r#"{{"Transfer":"transfer-{}"}}"#, HEX_STRING),
            format!(r#"{{"DeployInfo":"deploy-{}"}}"#, HEX_STRING),
            r#"{"EraInfo":"era-42"}"#.to_string(),
            format!(r#"{{"Balance":"balance-{}"}}"#, HEX_STRING),
            format!(r#"{{"Bid":"bid-{}"}}"#, HEX_STRING),
            format!(r#"{{"Withdraw":"withdraw-{}"}}"#, HEX_STRING),
            format!(r#"{{"Dictionary":"dictionary-{}"}}"#, HEX_STRING),
            format!(
                r#"{{"SystemContractRegistry":"system-contract-registry-{}"}}"#,
                base16::encode_lower(&SYSTEM_CONTRACT_REGISTRY_KEY_BYTES)
            ),
            format!(
                r#"{{"ChainspecRegistry":"chainspec-registry-{}"}}"#,
                base16::encode_lower(&CHAINSPEC_REGISTRY_KEY_BYTES)
            ),
            format!(r#"{{"Unbond":"unbond-{}"}}"#, HEX_STRING),
            format!(
                r#"{{"BlockEffectsRootHash":"block-effects-root-hash-{}"}}"#,
                BLOCK_HEIGHT
            ),
            format!(
                r#"{{"DeployApprovalsRootHash":"deploy-approvals-root-hash-{}"}}"#,
                BLOCK_HEIGHT
            ),
        ];

        assert_eq!(
            KEYS.len(),
            expected_json.len(),
            "There should be exactly one expected JSON string per test key"
        );

        for (key, expected_json_key) in KEYS.iter().zip(expected_json.iter()) {
            assert_eq!(serde_json::to_string(key).unwrap(), *expected_json_key);
        }
    }

    #[test]
    fn serialization_roundtrip_bincode() {
        for key in &KEYS {
            let encoded = bincode::serialize(key).unwrap();
            let decoded = bincode::deserialize(&encoded).unwrap();
            assert_eq!(key, &decoded);
        }
    }

    #[test]
    fn serialization_roundtrip_json() {
        let round_trip = |key: &Key| {
            let encoded = serde_json::to_string_pretty(key).unwrap();
            let decoded = serde_json::from_str(&encoded).unwrap();
            assert_eq!(key, &decoded);
        };

        for key in &KEYS {
            round_trip(key);
        }

        let zeros = [0; BLAKE2B_DIGEST_LENGTH];

        round_trip(&Key::Account(AccountHash::new(zeros)));
        round_trip(&Key::Hash(zeros));
        round_trip(&Key::URef(URef::new(zeros, AccessRights::READ)));
        round_trip(&Key::Transfer(TransferAddr::new(zeros)));
        round_trip(&Key::DeployInfo(DeployHash::new(zeros)));
        round_trip(&Key::EraInfo(EraId::from(0)));
        round_trip(&Key::Balance(URef::new(zeros, AccessRights::READ).addr()));
        round_trip(&Key::Bid(AccountHash::new(zeros)));
        round_trip(&Key::Withdraw(AccountHash::new(zeros)));
        round_trip(&Key::Unbond(AccountHash::new(zeros)));
        round_trip(&Key::Dictionary(zeros));
        round_trip(&Key::SystemContractRegistry);
        round_trip(&Key::ChainspecRegistry);
        round_trip(&Key::BlockEffectsRootHash {
            block_height: BLOCK_HEIGHT,
        });
        round_trip(&Key::DeployApprovalsRootHash {
            block_height: BLOCK_HEIGHT,
        });
    }
}
