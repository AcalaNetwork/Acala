#![cfg_attr(not(feature = "std"), no_std)]

use sr_primitives::Fixed64;

pub type Price = Fixed64;
pub type ExchangeRage = Fixed64;
pub type Ratio = Fixed64;

pub trait RiskManager<AccountId, CurrencyId, Amount, DebitAmount> {
	type Error: Into<&'static str>;

	fn required_collateral_ratio(currency_id: CurrencyId) -> Fixed64;

	fn check_position_adjustment(
		account_id: &AccountId,
		currency_id: CurrencyId,
		collaterals: Amount,
		debits: DebitAmount,
	) -> Result<(), Self::Error>;

	fn check_debit_cap(currency_id: CurrencyId, debits: DebitAmount) -> Result<(), Self::Error>;
}
