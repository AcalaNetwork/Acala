#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::convert::TryFrom;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	ACA = 0,
	AUSD = 1,
	DOT = 2,
	XBTC = 3,
	LDOT = 4,
	RENBTC = 5,
}

impl Into<Vec<u8>> for CurrencyId {
	fn into(self) -> Vec<u8> {
		use CurrencyId::*;
		match self {
			ACA => b"ACA".to_vec(),
			AUSD => b"AUSD".to_vec(),
			DOT => b"DOT".to_vec(),
			XBTC => b"XBTC".to_vec(),
			LDOT => b"LDOT".to_vec(),
			RENBTC => b"RENBTC".to_vec(),
		}
	}
}

impl TryFrom<Vec<u8>> for CurrencyId {
	type Error = ();
	fn try_from(v: Vec<u8>) -> Result<CurrencyId, ()> {
		match v.as_slice() {
			b"ACA" => Ok(CurrencyId::ACA),
			b"AUSD" => Ok(CurrencyId::AUSD),
			b"DOT" => Ok(CurrencyId::DOT),
			b"XBTC" => Ok(CurrencyId::XBTC),
			b"LDOT" => Ok(CurrencyId::LDOT),
			b"RENBTC" => Ok(CurrencyId::RENBTC),
			_ => Err(()),
		}
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AirDropCurrencyId {
	KAR = 0,
	ACA,
}

/// Counter for the number of eras that have passed.
pub type EraIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Signed version of Balance
pub type Amount = i128;

pub type AuctionId = u32;
