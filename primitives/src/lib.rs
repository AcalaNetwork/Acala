// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

pub mod bonding;
pub mod currency;
pub mod evm;
pub mod nft;
pub mod signature;
pub mod task;
pub mod testing;
pub mod unchecked_extrinsic;
pub use testing::*;

use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_core::U256;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	FixedU128, RuntimeDebug,
};
use sp_std::prelude::*;

pub use currency::{CurrencyId, DexShare, Lease, TokenSymbol};
pub use evm::{convert_decimals_from_evm, convert_decimals_to_evm};

#[cfg(test)]
mod tests;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on
/// the chain.
pub type Signature = signature::AcalaMultiSignature;

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

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, AccountIndex>;

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

/// Fee multiplier.
pub type Multiplier = FixedU128;

#[derive(
	Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, TypeInfo, Serialize, Deserialize,
)]
pub enum AuthoritysOriginId {
	Root,
	Treasury,
	HonzonTreasury,
	HomaTreasury,
	TreasuryReserve,
}

#[derive(
	Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, TypeInfo, Serialize, Deserialize,
)]
pub enum DataProviderId {
	Aggregated = 0,
	Acala = 1,
}

#[derive(
	Encode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, TypeInfo, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct TradingPair(CurrencyId, CurrencyId);

impl TradingPair {
	pub fn from_currency_ids(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> Option<Self> {
		if currency_id_a.is_trading_pair_currency_id()
			&& currency_id_b.is_trading_pair_currency_id()
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
	fn decode<I: parity_scale_codec::Input>(input: &mut I) -> sp_std::result::Result<Self, parity_scale_codec::Error> {
		let (first, second): (CurrencyId, CurrencyId) = Decode::decode(input)?;
		TradingPair::from_currency_ids(first, second)
			.ok_or_else(|| parity_scale_codec::Error::from("invalid currency id"))
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, Default, MaxEncodedLen, TypeInfo)]
pub struct Position {
	/// The amount of collateral.
	pub collateral: Balance,
	/// The amount of debit.
	pub debit: Balance,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, MaxEncodedLen, TypeInfo)]
#[repr(u8)]
pub enum ReserveIdentifier {
	CollatorSelection,
	EvmStorageDeposit,
	EvmDeveloperDeposit,
	Honzon,
	Nft,
	TransactionPayment,
	TransactionPaymentDeposit,

	// always the last, indicate number of variants
	Count,
}

/// Convert any type that implements Into<U256> into byte representation ([u8, 32])
pub fn to_bytes<T: Into<U256>>(value: T) -> [u8; 32] {
	Into::<[u8; 32]>::into(value.into())
}
