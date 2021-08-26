// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::upper_case_acronyms)]

pub mod currency;
pub mod evm;

use codec::{Decode, Encode, MaxEncodedLen};
use core::ops::Range;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, RuntimeDebug,
};
use sp_std::{convert::Into, prelude::*};

pub use currency::{CurrencyId, DexShare, TokenSymbol};

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
	Treasury,
	HonzonTreasury,
	HomaTreasury,
	TreasuryReserve,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum DataProviderId {
	Aggregated = 0,
	Acala = 1,
}

#[derive(Encode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TradingPair(CurrencyId, CurrencyId);

impl TradingPair {
	pub fn from_currency_ids(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> Option<Self> {
		if (currency_id_a.is_token_currency_id() || currency_id_a.is_erc20_currency_id())
			&& (currency_id_b.is_token_currency_id() || currency_id_b.is_erc20_currency_id())
			&& currency_id_a != currency_id_b
		{
			if currency_id_a > currency_id_b {
				Some(TradingPair(currency_id_b, currency_id_a))
			} else {
				Some(TradingPair(currency_id_a, currency_id_b))
			}
		} else {
			None
		}
	}

	pub fn first(&self) -> CurrencyId {
		self.0
	}

	pub fn second(&self) -> CurrencyId {
		self.1
	}

	pub fn dex_share_currency_id(&self) -> CurrencyId {
		CurrencyId::join_dex_share_currency_id(self.first(), self.second())
			.expect("shouldn't be invalid! guaranteed by construction")
	}
}

impl Decode for TradingPair {
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let (first, second): (CurrencyId, CurrencyId) = Decode::decode(input)?;
		TradingPair::from_currency_ids(first, second).ok_or_else(|| codec::Error::from("invalid currency id"))
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, MaxEncodedLen)]
#[repr(u8)]
pub enum ReserveIdentifier {
	CollatorSelection,
	EvmStorageDeposit,
	EvmDeveloperDeposit,
	Honzon,
	Nft,
	TransactionPayment,

	// always the last, indicate number of variants
	Count,
}

/// Ethereum precompiles
/// 0 - 0x400
/// Acala precompiles
/// 0x400 - 0x800
pub const PRECOMPILE_ADDRESS_START: u64 = 0x400;
/// Predeployed system contracts (except Mirrored ERC20)
/// 0x800 - 0x1000
pub const PREDEPLOY_ADDRESS_START: u64 = 0x800;
/// Mirrored Tokens (ensure length <= 4 bytes, encode to u32 will take the first 4 non-zero bytes)
/// 0x1000000
pub const MIRRORED_TOKENS_ADDRESS_START: u64 = 0x1000000;
/// Mirrored NFT (ensure length <= 4 bytes, encode to u32 will take the first 4 non-zero bytes)
/// 0x2000000
pub const MIRRORED_NFT_ADDRESS_START: u64 = 0x2000000;
/// Mirrored LP Tokens
/// 0x10000000000000000
pub const MIRRORED_LP_TOKENS_ADDRESS_START: u128 = 0x10000000000000000;
/// System contract address prefix
pub const SYSTEM_CONTRACT_ADDRESS_PREFIX: [u8; 11] = [0u8; 11];

/// CurrencyId to H160([u8; 20]) bit encoding rule.
///
/// Token
/// v[16] = 1 // MIRRORED_TOKENS_ADDRESS_START
/// - v[19] = token(1 byte)
///
/// DexShare
/// v[11] = 1 // MIRRORED_LP_TOKENS_ADDRESS_START
/// - v[12..16] = dex left(4 bytes)
/// - v[16..20] = dex right(4 bytes)
///
/// Erc20
/// - v[0..20] = evm address(20 bytes)
pub const H160_TYPE_TOKEN: u8 = 1;
pub const H160_TYPE_DEXSHARE: u8 = 1;
pub const H160_POSITION_TOKEN: usize = 19;
pub const H160_POSITION_DEXSHARE_LEFT: Range<usize> = 12..16;
pub const H160_POSITION_DEXSHARE_RIGHT: Range<usize> = 16..20;
pub const H160_POSITION_ERC20: Range<usize> = 0..20;
pub const H160_PREFIX_TOKEN: [u8; 19] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0];
pub const H160_PREFIX_DEXSHARE: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

pub type NFTBalance = u128;

pub type CashYieldIndex = u128;
