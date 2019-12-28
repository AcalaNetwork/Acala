#![cfg_attr(not(feature = "std"), no_std)]

use orml_utilities::FixedU128;
use sp_runtime::DispatchResult;

pub type Price = FixedU128;
pub type ExchangeRate = FixedU128;
pub type Ratio = FixedU128;
pub type Rate = FixedU128;

pub trait RiskManager<AccountId, CurrencyId, Amount, DebitAmount> {
	fn check_position_adjustment(
		account_id: &AccountId,
		currency_id: CurrencyId,
		collaterals: Amount,
		debits: DebitAmount,
	) -> DispatchResult;

	fn check_debit_cap(currency_id: CurrencyId, debits: DebitAmount) -> DispatchResult;
}

pub trait AuctionManager<AccountId> {
	type CurrencyId;
	type Balance;

	fn new_collateral_auction(
		who: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		bad_debt: Self::Balance,
	);
	fn new_debit_auction(amount: Self::Balance, fix: Self::Balance);
	fn new_surplus_auction(amount: Self::Balance);
	fn get_total_debit_in_auction() -> Self::Balance;
	fn get_total_target_in_auction() -> Self::Balance;
}

pub trait DexManager<AccountId, CurrencyId, Balance> {
	fn get_supply_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Balance;
	fn exchange_currency(
		who: AccountId,
		supply: (CurrencyId, Balance),
		target: (CurrencyId, Balance),
	) -> DispatchResult;
}

pub trait CDPTreasury<AccountId> {
	type Balance;

	fn on_system_debit(amount: Self::Balance);
	fn on_system_surplus(amount: Self::Balance);
	fn add_backed_debit(who: &AccountId, amount: Self::Balance) -> DispatchResult;
	fn sub_backed_debit(who: &AccountId, amount: Self::Balance) -> DispatchResult;
}
