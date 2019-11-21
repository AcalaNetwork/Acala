#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sr_primitives::RuntimeDebug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TokensCurrencyId {
	AUSD = 1,
	DOT,
	XBTC,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	ACA = 0,
	AUSD,
	DOT,
	XBTC,
}
