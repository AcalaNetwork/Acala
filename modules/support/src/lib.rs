#![cfg_attr(not(feature = "std"), no_std)]

use orml_utilities::FixedU128;

pub type Price = FixedU128;
pub type ExchangeRate = FixedU128;
pub type Ratio = FixedU128;
pub type Rate = FixedU128;

pub trait RiskManager<AccountId, CurrencyId, Amount, DebitAmount> {
	type Error: Into<&'static str>;

	fn check_position_adjustment(
		account_id: &AccountId,
		currency_id: CurrencyId,
		collaterals: Amount,
		debits: DebitAmount,
	) -> Result<(), Self::Error>;

	fn check_debit_cap(currency_id: CurrencyId, debits: DebitAmount) -> Result<(), Self::Error>;
}

pub trait AuctionManager<AccountId> {
	type CurrencyId;
	type Balance;
	type Amount;

	fn increase_surplus(increment: Self::Balance);

	fn new_collateral_auction(
		who: AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		bad_debt: Self::Balance,
	);
}

pub trait DexManager<AccountId, CurrencyId, Balance> {
	type Error: Into<&'static str>;

	fn get_supply_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Balance;
	fn exchange_currency(
		who: AccountId,
		supply: (CurrencyId, Balance),
		target: (CurrencyId, Balance),
	) -> Result<(), Self::Error>;
}
