#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, FullCodec, HasCompact};
use sp_runtime::{DispatchError, DispatchResult, FixedU128};
use sp_std::{
	cmp::{Eq, PartialEq},
	fmt::Debug,
	prelude::*,
};

pub mod homa;
pub use homa::{
	HomaProtocol, NomineesProvider, OnCommission, OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState,
	PolkadotBridgeType, PolkadotStakingLedger, PolkadotUnlockChunk,
};

pub type Price = FixedU128;
pub type ExchangeRate = FixedU128;
pub type Ratio = FixedU128;
pub type Rate = FixedU128;

pub trait RiskManager<AccountId, CurrencyId, Balance, DebitBalance> {
	fn get_bad_debt_value(currency_id: CurrencyId, debit_balance: DebitBalance) -> Balance;

	fn check_position_valid(
		currency_id: CurrencyId,
		collateral_balance: Balance,
		debit_balance: DebitBalance,
	) -> DispatchResult;

	fn check_debit_cap(currency_id: CurrencyId, total_debit_balance: DebitBalance) -> DispatchResult;
}

impl<AccountId, CurrencyId, Balance: Default, DebitBalance> RiskManager<AccountId, CurrencyId, Balance, DebitBalance>
	for ()
{
	fn get_bad_debt_value(_currency_id: CurrencyId, _debit_balance: DebitBalance) -> Balance {
		Default::default()
	}

	fn check_position_valid(
		_currency_id: CurrencyId,
		_collateral_balance: Balance,
		_debit_balance: DebitBalance,
	) -> DispatchResult {
		Ok(())
	}

	fn check_debit_cap(_currency_id: CurrencyId, _total_debit_balance: DebitBalance) -> DispatchResult {
		Ok(())
	}
}

pub trait AuctionManager<AccountId> {
	type CurrencyId;
	type Balance;
	type AuctionId: FullCodec + Debug + Clone + Eq + PartialEq;

	fn new_collateral_auction(
		who: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
	);
	fn new_debit_auction(amount: Self::Balance, fix: Self::Balance);
	fn new_surplus_auction(amount: Self::Balance);
	fn cancel_auction(id: Self::AuctionId) -> DispatchResult;

	fn get_total_collateral_in_auction(id: Self::CurrencyId) -> Self::Balance;
	fn get_total_surplus_in_auction() -> Self::Balance;
	fn get_total_debit_in_auction() -> Self::Balance;
	fn get_total_target_in_auction() -> Self::Balance;
}

pub trait DEXManager<AccountId, CurrencyId, Balance> {
	fn get_target_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_currency_amount: Balance,
	) -> Balance;

	fn get_supply_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Balance;

	fn exchange_currency(
		who: AccountId,
		supply_currency_id: CurrencyId,
		supply_amount: Balance,
		target_currency_id: CurrencyId,
		acceptable_target_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError>;

	fn get_exchange_slippage(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_amount: Balance,
	) -> Option<Ratio>;
}

impl<AccountId, CurrencyId, Balance> DEXManager<AccountId, CurrencyId, Balance> for ()
where
	Balance: Default,
{
	fn get_target_amount(
		_supply_currency_id: CurrencyId,
		_target_currency_id: CurrencyId,
		_supply_currency_amount: Balance,
	) -> Balance {
		Default::default()
	}

	fn get_supply_amount(
		_supply_currency_id: CurrencyId,
		_target_currency_id: CurrencyId,
		_target_currency_amount: Balance,
	) -> Balance {
		Default::default()
	}

	fn exchange_currency(
		_who: AccountId,
		_supply_currency_id: CurrencyId,
		_supply_amount: Balance,
		_target_currency_id: CurrencyId,
		_acceptable_target_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		Ok(Default::default())
	}

	fn get_exchange_slippage(
		_supply_currency_id: CurrencyId,
		_target_currency_id: CurrencyId,
		_supply_amount: Balance,
	) -> Option<Ratio> {
		None
	}
}

/// An abstraction of cdp treasury for Honzon Protocol.
pub trait CDPTreasury<AccountId> {
	type Balance;
	type CurrencyId;

	/// get surplus amount of cdp treasury
	fn get_surplus_pool() -> Self::Balance;

	/// get debit amount of cdp treasury
	fn get_debit_pool() -> Self::Balance;

	/// get collateral assets amount of cdp treasury
	fn get_total_collaterals(id: Self::CurrencyId) -> Self::Balance;

	/// calculate the proportion of specific debit amount for the whole system
	fn get_debit_proportion(amount: Self::Balance) -> Ratio;

	/// issue debit for cdp treasury
	fn on_system_debit(amount: Self::Balance) -> DispatchResult;

	/// issue surplus(stable coin) for cdp treasury
	fn on_system_surplus(amount: Self::Balance) -> DispatchResult;

	/// issue debit to `who`
	/// if backed flag is true, means the debit to issue is backed on some
	/// assets, otherwise will increase same amount of debit to system debit.
	fn issue_debit(who: &AccountId, debit: Self::Balance, backed: bool) -> DispatchResult;

	/// burn debit(stable coin) of `who`
	fn burn_debit(who: &AccountId, debit: Self::Balance) -> DispatchResult;

	/// deposit surplus(stable coin) to cdp treasury by `from`
	fn deposit_surplus(from: &AccountId, surplus: Self::Balance) -> DispatchResult;

	/// deposit collateral assets to cdp treasury by `who`
	fn deposit_collateral(from: &AccountId, currency_id: Self::CurrencyId, amount: Self::Balance) -> DispatchResult;

	/// withdraw collateral assets of cdp treasury to `who`
	fn withdraw_collateral(to: &AccountId, currency_id: Self::CurrencyId, amount: Self::Balance) -> DispatchResult;
}

pub trait CDPTreasuryExtended<AccountId>: CDPTreasury<AccountId> {
	fn swap_collateral_to_stable(
		currency_id: Self::CurrencyId,
		supply_amount: Self::Balance,
		target_amount: Self::Balance,
	) -> sp_std::result::Result<Self::Balance, DispatchError>;
	fn create_collateral_auctions(
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		refund_receiver: AccountId,
	);
}

pub trait PriceProvider<CurrencyId> {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<Price>;
	fn get_price(currency_id: CurrencyId) -> Option<Price>;
	fn lock_price(currency_id: CurrencyId);
	fn unlock_price(currency_id: CurrencyId);
}

pub trait ExchangeRateProvider {
	fn get_exchange_rate() -> ExchangeRate;
}

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnEmergencyShutdown {
	fn on_emergency_shutdown();
}
