use codec::{Decode, Encode};
use sr_primitives::RuntimeDebug;

#[derive(RuntimeDebug, Encode, Decode, Clone, Copy, Eq, PartialEq)]
pub enum TokensCurrencyId {
	AUSD = 1,
	DOT,
	XBTC,
}

#[derive(RuntimeDebug, Encode, Decode, Clone, Copy, Eq, PartialEq)]
pub enum CurrencyId {
	ACA = 0,
	AUSD,
	DOT,
	XBTC,
}
