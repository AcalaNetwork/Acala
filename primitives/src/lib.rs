#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::upper_case_acronyms)]

pub mod currency;
pub mod evm;
pub mod mocks;

use codec::{Decode, Encode};
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, RuntimeDebug,
};
use sp_std::{convert::Into, prelude::*};

pub use currency::{CurrencyId, TokenSymbol};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on
/// the chain.
pub type Signature = MultiSignature;

/// Alias to the public key used for this chain, actually a `MultiSigner`. Like
/// the signature, this also isn't a fixed size when encoded, as different
/// cryptos have different size public keys.
pub type AccountPublic = <Signature as Verify>::Signer;

/// Alias to the opaque account ID type for this chain, actually a
/// `AccountId32`. This is always 32 bytes.
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of
/// them.
pub type AccountIndex = u32;

/// Index of a transaction in the chain. 32-bit should be plenty.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An instant or duration in time.
pub type Moment = u64;

/// Counter for the number of eras that have passed.
pub type EraIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Signed version of Balance
pub type Amount = i128;

/// Auction ID
pub type AuctionId = u32;

/// Share type
pub type Share = u128;

/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// Block ID.
pub type BlockId = generic::BlockId<Block>;

/// Opaque, encoded, unchecked extrinsic.
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AirDropCurrencyId {
	KAR = 0,
	ACA,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AuthoritysOriginId {
	Root,
	AcalaTreasury,
	HonzonTreasury,
	HomaTreasury,
	DSWF,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum DataProviderId {
	Aggregated = 0,
	Acala = 1,
	Band = 2,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TradingPair(pub CurrencyId, pub CurrencyId);

impl TradingPair {
	pub fn new(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> Self {
		if currency_id_a > currency_id_b {
			TradingPair(currency_id_b, currency_id_a)
		} else {
			TradingPair(currency_id_a, currency_id_b)
		}
	}

	pub fn from_token_currency_ids(currency_id_0: CurrencyId, currency_id_1: CurrencyId) -> Option<Self> {
		match currency_id_0.is_token_currency_id() && currency_id_1.is_token_currency_id() {
			true if currency_id_0 > currency_id_1 => Some(TradingPair(currency_id_1, currency_id_0)),
			true if currency_id_0 < currency_id_1 => Some(TradingPair(currency_id_0, currency_id_1)),
			_ => None,
		}
	}

	pub fn get_dex_share_currency_id(&self) -> Option<CurrencyId> {
		CurrencyId::join_dex_share_currency_id(self.0, self.1)
	}
}

/// Ethereum precompiles
/// 0 - 0x400
/// Acala precompiles
/// 0x400 - 0x800
pub const PRECOMPILE_ADDRESS_START: u64 = 0x400;
/// Predeployed system contracts (except Mirrored ERC20)
/// 0x800 - 0x1000
pub const PREDEPLOY_ADDRESS_START: u64 = 0x800;
/// Mirrored Tokens
/// 0x01000000
pub const MIRRORED_TOKENS_ADDRESS_START: u64 = 0x01000000;
/// Mirrored NFT
/// 0x02000000
pub const MIRRORED_NFT_ADDRESS_START: u64 = 0x02000000;

pub type NFTBalance = u128;
