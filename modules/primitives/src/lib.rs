#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::convert::{TryFrom, TryInto};

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
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AirDropCurrencyId {
	KAR = 0,
	ACA,
}

impl TryFrom<u16> for CurrencyId {
	type Error = ();

	fn try_from(a: u16) -> Result<Self, Self::Error> {
		match a {
			0u16 => Ok(CurrencyId::ACA),
			1u16 => Ok(CurrencyId::AUSD),
			2u16 => Ok(CurrencyId::DOT),
			3u16 => Ok(CurrencyId::XBTC),
			4u16 => Ok(CurrencyId::LDOT),
			_ => Err(()),
		}
	}
}

impl TryInto<u16> for CurrencyId {
	type Error = ();

	fn try_into(self) -> Result<u16, Self::Error> {
		Ok(self as u16)
	}
}
