#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;
use rstd::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
};
use sr_primitives::traits::MaybeSerializeDeserialize;
use sr_primitives::Fixed64;
use traits::arithmetic::{self, Signed};

pub type Price = Fixed64;
pub type ExchangeRage = Fixed64;
pub type Ratio = Fixed64;

pub trait RiskManager<CurrencyId, Balance, DebitBalance> {
	type Error: Into<&'static str>;
	type Amount: Signed
		+ TryInto<Balance>
		+ TryFrom<Balance>
		+ arithmetic::SimpleArithmetic
		+ Codec
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ Default;
	type DebitAmount: Signed
		+ TryInto<Balance>
		+ TryFrom<Balance>
		+ arithmetic::SimpleArithmetic
		+ Codec
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ Default;

	fn required_collateral_ratio(currency_id: CurrencyId) -> Fixed64;
	fn check_position_adjustment(
		currency_id: CurrencyId,
		collaterals: Self::Amount,
		debits: Self::DebitAmount,
	) -> Result<(), Self::Error>;
}
