#![cfg_attr(not(feature = "std"), no_std)]

use sr_primitives::Fixed64;

pub type Price = Fixed64;
pub type ExchangeRage = Fixed64;
pub type Ratio = Fixed64;

pub struct SignedBalance<T> {
	balance: T,
}

pub trait RiskManager<CurrencyId, Balance, DebitBalance> {
	type Error;

	fn required_collateral_ratio(currency_id: CurrencyId) -> Fixed64;
	fn check_position_adjustment(
		currency_id: CurrencyId,
		collaterals: SignedBalance<Balance>,
		debits: SignedBalance<DebitBalance>,
	) -> Result<(), Self::Error>;
}
