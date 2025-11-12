// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Common runtime benchmarking code.

use crate::{
	AcalaOracle, AccountId, AccountIndex, AggregatedDex, Amount, AssetRegistry, Auction, AuctionId, AuctionManager,
	AuctionTimeToClose, Aura, Balance, CdpEngine, CdpTreasury, Currencies, CurrencyId, Dex, DexOracle,
	EmergencyShutdown, EraIndex, EvmTask, ExistentialDeposits, GetLiquidCurrencyId, GetNativeCurrencyId,
	GetStableCurrencyId, GetStakingCurrencyId, Homa, HomaValidatorList, MinimumCount, MinimumDebitValue, Moment,
	NativeTokenExistentialDeposit, OperatorMembershipAcala, Parameters, Permill, Price, Rate, Ratio, RawOrigin,
	Runtime, RuntimeOrigin, RuntimeParameters, ScheduledTasks, StableAsset, System, Timestamp, TradingPair,
	TreasuryAccount, ACA, DOT, EVM, LCDOT, LDOT, MILLISECS_PER_BLOCK,
};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::str::FromStr;
use frame_benchmarking::account;
use frame_support::{assert_ok, traits::fungibles, traits::Contains, traits::OnInitialize};
use frame_system::pallet_prelude::BlockNumberFor;
use module_aggregated_dex::SwapPath;
use module_support::AddressMapping;
use module_support::Erc20InfoMapping;
use module_support::{AuctionManager as AuctionManagerTrait, CDPTreasury};
use orml_traits::Change;
use orml_traits::{GetByKey, MultiCurrency, MultiCurrencyExtended};
use parity_scale_codec::Encode;
use primitives::{currency::AssetMetadata, evm::EvmAddress, AuthoritysOriginId};
use runtime_common::TokenInfo;
use sp_consensus_aura::AURA_ENGINE_ID;
use sp_core::Get;
use sp_runtime::{
	traits::{AccountIdConversion, AccountIdLookup, One, StaticLookup, UniqueSaturatedInto},
	Digest, DigestItem, FixedPointNumber, FixedU128, MultiAddress, SaturatedConversion,
};
use sp_std::vec;

/// Helper struct for benchmarking.
pub struct BenchmarkHelper<T>(sp_std::marker::PhantomData<T>);

/// Instance helper struct for benchmarking.
pub struct BenchmarkInstanceHelper<T, I>(sp_std::marker::PhantomData<(T, I)>);

impl<T, I> orml_oracle::BenchmarkHelper<T::OracleKey, T::OracleValue, T::MaxFeedValues>
	for BenchmarkInstanceHelper<T, I>
where
	T: orml_oracle::Config<I, OracleKey = CurrencyId, OracleValue = Price>,
{
	fn get_currency_id_value_pairs() -> sp_runtime::BoundedVec<(T::OracleKey, T::OracleValue), T::MaxFeedValues> {
		sp_runtime::BoundedVec::try_from(vec![
			(STAKING, FixedU128::saturating_from_rational(1, 1)),
			(LIQUID, FixedU128::saturating_from_rational(2, 1)),
			(STABLECOIN, FixedU128::saturating_from_rational(3, 1)),
		])
		.unwrap()
	}
}

impl<T> module_aggregated_dex::BenchmarkHelper<AccountId, CurrencyId, Balance> for BenchmarkHelper<T>
where
	T: module_aggregated_dex::Config,
{
	fn setup_currency_lists() -> Vec<CurrencyId> {
		[NATIVE, STABLECOIN, LIQUID, STAKING].to_vec()
	}
	// return (path, supply_amount, target_amount)
	fn setup_dex(u: u32, taker: AccountId) -> Option<(Vec<CurrencyId>, Balance, Balance)> {
		let maker: AccountId = account("maker", 0, 0);

		let currency_list = Self::setup_currency_lists();
		let mut path: Vec<CurrencyId> = vec![];

		for i in 1..u {
			if i == 1 {
				let cur0 = currency_list[0];
				let cur1 = currency_list[1];
				path.push(cur0);
				path.push(cur1);
				assert_ok!(inject_liquidity(
					maker.clone(),
					cur0,
					cur1,
					10_000 * dollar(cur0),
					10_000 * dollar(cur1),
					false,
				));
			} else {
				path.push(currency_list[i as usize]);
				assert_ok!(inject_liquidity(
					maker.clone(),
					currency_list[i as usize - 1],
					currency_list[i as usize],
					10_000 * dollar(currency_list[i as usize - 1]),
					10_000 * dollar(currency_list[i as usize]),
					false,
				));
			}
		}

		set_balance(path[0], &taker, 10_000 * dollar(path[0]));

		Some((path.clone(), 1_000 * dollar(path[0]), 10 * dollar(path[path.len() - 1])))
	}
}

impl<T> module_asset_registry::BenchmarkHelper for BenchmarkHelper<T>
where
	T: module_asset_registry::Config,
{
	fn setup_deploy_contract() -> Option<EvmAddress> {
		deploy_contract();
		Some(erc20_address())
	}
}

impl<T> module_auction_manager::BenchmarkHelper for BenchmarkHelper<T>
where
	T: module_auction_manager::Config,
{
	fn setup() -> Option<AuctionId> {
		let auction_id: AuctionId = 0;
		let bidder: AccountId = account("bidder", 0, 0);
		let funder: AccountId = account("funder", 0, 0);

		// set balance
		assert_ok!(Currencies::deposit(STABLECOIN, &bidder, 80 * dollar(STABLECOIN)));
		assert_ok!(Currencies::deposit(STAKING, &funder, dollar(STAKING)));
		assert_ok!(CdpTreasury::deposit_collateral(&funder, STAKING, dollar(STAKING)));

		// feed price
		feed_price(vec![(STAKING, Price::saturating_from_integer(120))]);

		// create collateral auction
		assert_ok!(AuctionManager::new_collateral_auction(
			&funder,
			STAKING,
			dollar(STAKING),
			100 * dollar(STABLECOIN)
		));

		// bid collateral auction
		assert_ok!(AuctionManager::collateral_auction_bid_handler(
			1,
			auction_id,
			(bidder, 80 * dollar(STABLECOIN)),
			None
		));

		// shutdown
		assert_ok!(EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into()));

		Some(auction_id)
	}
}

impl<T> module_cdp_engine::BenchmarkHelper<CurrencyId, BlockNumberFor<T>, MultiAddress<AccountId, AccountIndex>>
	for BenchmarkHelper<T>
where
	T: module_cdp_engine::Config,
{
	fn setup_on_initialize(c: u32) -> Option<BlockNumberFor<T>> {
		let owner: AccountId = account("owner", 0, 0);
		let currency_ids = get_benchmarking_collateral_currency_ids();
		let min_debit_value = T::MinimumDebitValue::get();
		let debit_exchange_rate = T::DefaultDebitExchangeRate::get();
		let min_debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;

		// feed price
		let mut feed_data: Vec<(CurrencyId, Price)> = vec![];
		for i in 0..c.min(currency_ids.len() as u32) {
			let currency_id = currency_ids[i as usize];
			let collateral_price = Price::one();
			feed_data.push((currency_id, collateral_price));
		}
		feed_price(feed_data);

		for i in 0..c.min(currency_ids.len() as u32) {
			let currency_id = currency_ids[i as usize];
			if matches!(currency_id, CurrencyId::StableAssetPoolToken(_)) {
				continue;
			}
			let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(STABLECOIN))
				.saturating_mul_int(collateral_value);

			let ed = if currency_id == NATIVE {
				NativeTokenExistentialDeposit::get()
			} else {
				ExistentialDeposits::get(&currency_id)
			};

			// set balance
			set_balance(currency_id, &owner, collateral_amount + ed);

			assert_ok!(CdpEngine::set_collateral_params(
				RawOrigin::Root.into(),
				currency_id,
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(0, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(0, 100))),
				Change::NewValue(min_debit_value * 100),
			));

			// adjust position
			assert_ok!(CdpEngine::adjust_position(
				&owner,
				currency_id,
				collateral_amount.try_into().unwrap(),
				min_debit_amount
			));
		}

		set_block_number_timestamp(2, MILLISECS_PER_BLOCK);
		CdpEngine::on_initialize(2);

		set_block_number_timestamp(3, MILLISECS_PER_BLOCK * 2);
		Some(3u32.into())
	}
	fn setup_liquidate_by_auction(b: u32) -> Option<(CurrencyId, MultiAddress<AccountId, AccountIndex>)> {
		let owner: AccountId = account("owner", 0, 0);
		let owner_lookup = AccountIdLookup::unlookup(owner.clone());
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(STAKING);
		let collateral_price = Price::one(); // 1 USD
		let min_debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;
		let collateral_amount =
			Price::saturating_from_rational(dollar(STAKING), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(STAKING, &owner, collateral_amount + ExistentialDeposits::get(&STAKING));

		// feed price
		feed_price(vec![(STAKING, collateral_price)]);

		// set risk params
		assert_ok!(CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			STAKING,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		));

		let auction_size = collateral_amount / b as u128;
		// adjust auction size so we hit MaxAuctionCount
		assert_ok!(CdpTreasury::set_expected_collateral_auction_size(
			RawOrigin::Root.into(),
			STAKING,
			auction_size
		));
		// adjust position
		assert_ok!(CdpEngine::adjust_position(
			&owner,
			STAKING,
			collateral_amount.try_into().unwrap(),
			min_debit_amount
		));

		// modify liquidation rate to make the cdp unsafe
		assert_ok!(CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			STAKING,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(1000, 100))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		Some((STAKING, owner_lookup))
	}
	fn setup_liquidate_by_dex() -> Option<(CurrencyId, MultiAddress<AccountId, AccountIndex>)> {
		let owner: AccountId = account("owner", 0, 0);
		let owner_lookup = AccountIdLookup::unlookup(owner.clone());
		let funder: AccountId = account("funder", 0, 0);
		let debit_value = 100 * dollar(STABLECOIN);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(LIQUID);
		let debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 2 * debit_value;
		let collateral_amount =
			Price::saturating_from_rational(dollar(LIQUID), dollar(STABLECOIN)).saturating_mul_int(collateral_value);
		let collateral_price = Price::one(); // 1 USD

		set_balance(
			LIQUID,
			&owner,
			(10 * collateral_amount) + ExistentialDeposits::get(&LIQUID),
		);
		assert_ok!(inject_liquidity(
			funder.clone(),
			LIQUID,
			STAKING,
			10_000 * dollar(LIQUID),
			10_000 * dollar(STAKING),
			false,
		));
		assert_ok!(inject_liquidity(
			funder,
			STAKING,
			STABLECOIN,
			10_000 * dollar(STAKING),
			10_000 * dollar(STABLECOIN),
			false,
		));

		// feed price
		feed_price(vec![(STAKING, collateral_price)]);

		// set risk params
		assert_ok!(CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			LIQUID,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		));

		// adjust position
		assert_ok!(CdpEngine::adjust_position(
			&owner,
			LIQUID,
			(10 * collateral_amount).try_into().unwrap(),
			debit_amount
		));

		// modify liquidation rate to make the cdp unsafe
		assert_ok!(CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			LIQUID,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(1000, 100))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		Some((LIQUID, owner_lookup))
	}
	fn setup_settle() -> Option<(CurrencyId, MultiAddress<AccountId, AccountIndex>)> {
		let owner: AccountId = account("owner", 0, 0);
		let owner_lookup = AccountIdLookup::unlookup(owner.clone());
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(STAKING);
		let collateral_price = Price::one(); // 1 USD
		let min_debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;
		let collateral_amount = Price::saturating_from_rational(1_000 * dollar(STAKING), 1000 * dollar(STABLECOIN))
			.saturating_mul_int(collateral_value);

		// set balance
		set_balance(STAKING, &owner, collateral_amount + ExistentialDeposits::get(&STAKING));

		// feed price
		feed_price(vec![(STAKING, collateral_price)]);

		// set risk params
		assert_ok!(CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			STAKING,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		));

		// adjust position
		assert_ok!(CdpEngine::adjust_position(
			&owner,
			STAKING,
			collateral_amount.try_into().unwrap(),
			min_debit_amount
		));

		// shutdown
		assert_ok!(EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into()));
		Some((STAKING, owner_lookup))
	}
}

impl<T> module_cdp_treasury::BenchmarkHelper<AccountId, CurrencyId> for BenchmarkHelper<T>
where
	T: module_cdp_treasury::Config,
{
	fn setup_dex_pools(caller: AccountId) -> Option<CurrencyId> {
		assert_ok!(initialize_swap_pools(caller));
		Some(STAKING)
	}
}

impl<T> module_currencies::BenchmarkHelper<AccountId, CurrencyId, Balance> for BenchmarkHelper<T>
where
	T: module_currencies::Config,
{
	fn setup_get_staking_currency_id_and_amount() -> Option<(CurrencyId, Balance)> {
		Some((STAKING, 100 * dollar(STAKING)))
	}
	fn setup_get_treasury_account() -> Option<AccountId> {
		Some(TreasuryAccount::get())
	}
}

impl<T> module_dex_oracle::BenchmarkHelper<CurrencyId, Moment> for BenchmarkHelper<T>
where
	T: module_dex_oracle::Config,
{
	fn setup_on_initialize(n: u32, u: u32) {
		let caller: AccountId = account("caller", 0, 0);

		let trading_pair_list = vec![
			TradingPair::from_currency_ids(NATIVE, STABLECOIN).unwrap(),
			TradingPair::from_currency_ids(NATIVE, STAKING).unwrap(),
			TradingPair::from_currency_ids(STAKING, STABLECOIN).unwrap(),
		];

		for i in 0..n {
			let trading_pair = trading_pair_list[i as usize];
			assert_ok!(inject_liquidity(
				caller.clone(),
				trading_pair.first(),
				trading_pair.second(),
				dollar(trading_pair.first()) * 100,
				dollar(trading_pair.second()) * 1000,
				false,
			));
			assert_ok!(DexOracle::enable_average_price(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second(),
				240000
			));
		}
		for j in 0..u.min(n) {
			let update_pair = trading_pair_list[j as usize];
			assert_ok!(DexOracle::update_average_price_interval(
				RawOrigin::Root.into(),
				update_pair.first(),
				update_pair.second(),
				24000
			));
		}
		set_block_number_timestamp(1, 24000);
	}

	fn setup_inject_liquidity() -> Option<(CurrencyId, CurrencyId, Moment)> {
		let caller: AccountId = account("caller", 0, 0);

		assert_ok!(inject_liquidity(
			caller.clone(),
			NATIVE,
			STABLECOIN,
			dollar(NATIVE) * 100,
			dollar(STABLECOIN) * 1000,
			false,
		));

		Some((NATIVE, STABLECOIN, 24000))
	}
}

impl<T> module_earning::BenchmarkHelper for BenchmarkHelper<T>
where
	T: module_earning::Config,
{
	fn setup_parameter_store() {
		assert_ok!(Parameters::set_parameter(
			RawOrigin::Root.into(),
			RuntimeParameters::Earning(module_earning::Parameters::InstantUnstakeFee(
				module_earning::InstantUnstakeFee,
				Some(Permill::from_percent(10))
			))
		));
	}
}

impl<T> module_emergency_shutdown::BenchmarkHelper for BenchmarkHelper<T>
where
	T: module_emergency_shutdown::Config,
{
	fn setup_feed_price(c: u32) {
		let currency_ids = get_benchmarking_collateral_currency_ids();

		let funder: AccountId = account("funder", 0, 0);
		let mut values = vec![];

		for i in 0..c.min(currency_ids.len() as u32) {
			let currency_id = currency_ids[i as usize];
			if matches!(currency_id, CurrencyId::StableAssetPoolToken(_)) {
				continue;
			}
			values.push((currency_id, Price::one()));
			set_balance(currency_id, &funder, 100 * dollar(currency_id));
			assert_ok!(CdpTreasury::deposit_collateral(
				&funder,
				currency_id,
				100 * dollar(currency_id)
			));
		}
		feed_price(values);
	}
}

impl<T> module_homa_validator_list::BenchmarkHelper<EraIndex> for BenchmarkHelper<T>
where
	T: module_homa_validator_list::Config,
{
	fn setup_homa_bump_era(era_index: EraIndex) {
		assert_ok!(Homa::force_bump_current_era(RawOrigin::Root.into(), era_index));
	}
}

impl<T> module_idle_scheduler::BenchmarkHelper<ScheduledTasks> for BenchmarkHelper<T>
where
	T: module_idle_scheduler::Config,
{
	fn setup_schedule_task() -> Option<ScheduledTasks> {
		Some(ScheduledTasks::EvmTask(EvmTask::Remove {
			caller: Default::default(),
			contract: Default::default(),
			maintainer: Default::default(),
		}))
	}
}

impl<T> module_nominees_election::BenchmarkHelper<EraIndex, AccountId, AccountId> for BenchmarkHelper<T>
where
	T: module_nominees_election::Config,
{
	fn setup_homa_bump_era(era_index: EraIndex) {
		assert_ok!(Homa::force_bump_current_era(RawOrigin::Root.into(), era_index));
	}
	fn setup_homa_validators(caller: AccountId, targets: Vec<AccountId>) {
		for validator in targets.iter() {
			assert_ok!(HomaValidatorList::bond(
				RawOrigin::Signed(caller.clone()).into(),
				validator.clone(),
				<Runtime as module_homa_validator_list::Config>::ValidatorInsuranceThreshold::get()
			));
		}
	}
}

impl<T> module_prices::BenchmarkHelper<CurrencyId> for BenchmarkHelper<T>
where
	T: module_prices::Config,
{
	fn setup_feed_price() -> Option<CurrencyId> {
		feed_price(vec![(STAKING, dollar(STAKING).into())]);
		Some(STAKING)
	}
}

impl<T> orml_tokens::BenchmarkHelper<T::CurrencyId, T::Balance> for BenchmarkHelper<T>
where
	T: orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>,
{
	fn get_currency_id_and_amount() -> Option<(T::CurrencyId, T::Balance)> {
		Some((STAKING, dollar(STAKING)))
	}
}

impl<T> orml_vesting::BenchmarkHelper<T::AccountId, <T as pallet_balances::Config>::Balance> for BenchmarkHelper<T>
where
	T: frame_system::Config<AccountId = AccountId> + pallet_balances::Config<Balance = Balance> + orml_vesting::Config,
{
	fn get_vesting_account_and_amount() -> Option<(T::AccountId, <T as pallet_balances::Config>::Balance)> {
		Some((get_vesting_account(), dollar(NATIVE)))
	}
}

impl<T> orml_auction::BenchmarkHelper<BlockNumberFor<T>, T::AccountId, T::Balance> for BenchmarkHelper<T>
where
	T: orml_auction::Config<AccountId = AccountId, Balance = Balance>,
{
	fn setup_bid() -> Option<(T::AccountId, T::Balance)> {
		let bidder: AccountId = account("bidder", 0, 0);
		let previous_bidder: AccountId = account("previous_bidder", 0, 0);
		let funder: AccountId = account("funder", 0, 0);
		let collateral_amount: Balance = 100 * dollar(STAKING);
		let target_amount: Balance = 10_000 * dollar(STABLECOIN);
		let previous_bid_price: Balance = 5_000u128 * dollar(STABLECOIN);
		let bid_price: Balance = 10_000u128 * dollar(STABLECOIN);
		let auction_id: AuctionId = 0;

		set_balance(STAKING, &funder, collateral_amount);
		set_balance(STABLECOIN, &bidder, bid_price);
		set_balance(STABLECOIN, &previous_bidder, previous_bid_price);
		assert_ok!(<CdpTreasury as CDPTreasury<_>>::deposit_collateral(
			&funder,
			STAKING,
			collateral_amount
		));
		assert_ok!(AuctionManager::new_collateral_auction(
			&funder,
			STAKING,
			collateral_amount,
			target_amount
		));
		assert_ok!(Auction::bid(
			RawOrigin::Signed(previous_bidder).into(),
			auction_id,
			previous_bid_price
		));

		Some((bidder, bid_price))
	}

	fn setup_on_finalize(rand: u32) -> Option<BlockNumberFor<T>> {
		let bidder = account("bidder", 0, 0);
		let funder = account("funder", 0, 0);
		let collateral_amount = 100 * dollar(STAKING);
		let target_amount = 10_000 * dollar(STABLECOIN);
		let bid_price = 5_000u128 * dollar(STABLECOIN);

		System::set_block_number(1);
		for auction_id in 0..rand {
			set_balance(STAKING, &funder, collateral_amount);
			assert_ok!(<CdpTreasury as CDPTreasury<_>>::deposit_collateral(
				&funder,
				STAKING,
				collateral_amount
			));
			assert_ok!(AuctionManager::new_collateral_auction(
				&funder,
				STAKING,
				collateral_amount,
				target_amount
			));
			set_balance(STABLECOIN, &bidder, bid_price);
			assert_ok!(Auction::bid(
				RawOrigin::Signed(bidder.clone()).into(),
				auction_id,
				bid_price
			));
		}
		Some((System::block_number() + AuctionTimeToClose::get()).into())
	}
}

impl<T> orml_authority::BenchmarkHelper<T::AsOriginId> for BenchmarkHelper<T>
where
	T: orml_authority::Config<AsOriginId = AuthoritysOriginId>,
{
	fn get_as_origin_id() -> Option<T::AsOriginId> {
		Some(AuthoritysOriginId::Root)
	}
}

pub const NATIVE: CurrencyId = GetNativeCurrencyId::get();
pub const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
pub const LIQUID: CurrencyId = GetLiquidCurrencyId::get();
pub const STAKING: CurrencyId = GetStakingCurrencyId::get();

fn get_vesting_account() -> super::AccountId {
	super::TreasuryPalletId::get().into_account_truncating()
}

fn get_benchmarking_collateral_currency_ids() -> Vec<CurrencyId> {
	vec![ACA, DOT, LDOT, CurrencyId::StableAssetPoolToken(0)]
}

pub fn register_native_asset(assets: Vec<CurrencyId>) {
	assets.iter().for_each(|asset| {
		let ed = if *asset == GetNativeCurrencyId::get() {
			NativeTokenExistentialDeposit::get()
		} else {
			ExistentialDeposits::get(&asset)
		};
		assert_ok!(AssetRegistry::register_native_asset(
			RuntimeOrigin::root(),
			*asset,
			Box::new(AssetMetadata {
				name: asset.name().unwrap().as_bytes().to_vec(),
				symbol: asset.symbol().unwrap().as_bytes().to_vec(),
				decimals: asset.decimals().unwrap(),
				minimal_balance: ed,
			})
		));
	});
}

pub fn set_balance(currency_id: CurrencyId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

pub fn feed_price(prices: Vec<(CurrencyId, Price)>) {
	for i in 0..MinimumCount::get() {
		let oracle: AccountId = account("oracle", 0, i);
		if !OperatorMembershipAcala::contains(&oracle) {
			assert_ok!(OperatorMembershipAcala::add_member(
				RawOrigin::Root.into(),
				MultiAddress::Id(oracle.clone())
			));
		}
		assert_ok!(AcalaOracle::feed_values(
			RawOrigin::Signed(oracle).into(),
			prices.to_vec().try_into().unwrap()
		));
	}
}

pub fn set_block_number_timestamp(block_number: u32, timestamp: u64) {
	let slot = timestamp / Aura::slot_duration();
	let digest = Digest {
		logs: vec![DigestItem::PreRuntime(AURA_ENGINE_ID, slot.encode())],
	};
	System::initialize(&block_number, &Default::default(), &digest);
	Aura::on_initialize(block_number);
	Timestamp::set_timestamp(timestamp);
}

#[allow(dead_code)]
pub fn set_balance_fungibles(currency_id: CurrencyId, who: &AccountId, balance: Balance) {
	assert_ok!(<orml_tokens::Pallet<Runtime> as fungibles::Mutate<AccountId>>::mint_into(currency_id, who, balance));
}

pub fn dollar(currency_id: CurrencyId) -> Balance {
	if matches!(currency_id, CurrencyId::Token(_))
		&& module_asset_registry::EvmErc20InfoMapping::<Runtime>::decimals(currency_id).is_none()
	{
		register_native_asset(vec![currency_id]);
	}
	if let Some(decimals) = module_asset_registry::EvmErc20InfoMapping::<Runtime>::decimals(currency_id) {
		10u128.saturating_pow(decimals.into())
	} else {
		panic!("{:?} not support decimals", currency_id);
	}
}

pub fn inject_liquidity(
	maker: AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
	deposit: bool,
) -> Result<(), &'static str> {
	// set balance
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_a,
		&maker,
		max_amount_a.unique_saturated_into(),
	)?;
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_b,
		&maker,
		max_amount_b.unique_saturated_into(),
	)?;

	let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id_a, currency_id_b);

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		deposit,
	)?;
	Ok(())
}

pub fn register_stable_asset() {
	let asset_metadata = AssetMetadata {
		name: b"Token Name".to_vec(),
		symbol: b"TN".to_vec(),
		decimals: 12,
		minimal_balance: 1,
	};
	assert_ok!(AssetRegistry::register_stable_asset(
		RawOrigin::Root.into(),
		Box::new(asset_metadata.clone())
	));
}

pub fn create_stable_pools(assets: Vec<CurrencyId>, precisions: Vec<u128>, initial_a: u128) {
	let pool_asset = CurrencyId::StableAssetPoolToken(0);
	let mint_fee = 2u128;
	let swap_fee = 3u128;
	let redeem_fee = 5u128;
	let fee_recipient: AccountId = account("fee", 0, 0);
	let yield_recipient: AccountId = account("yield", 1, 0);

	register_stable_asset();
	assert_ok!(StableAsset::create_pool(
		RawOrigin::Root.into(),
		pool_asset,
		assets,
		precisions,
		mint_fee,
		swap_fee,
		redeem_fee,
		initial_a,
		fee_recipient,
		yield_recipient,
		1000000000000000000u128,
	));
}

/// Initializes all pools used in AggregatedDex `Swap` for trading to stablecoin
pub fn initialize_swap_pools(maker: AccountId) -> Result<(), &'static str> {
	// Inject liquidity into all possible `AlternativeSwapPathJointList`
	inject_liquidity(
		maker.clone(),
		LIQUID,
		STABLECOIN,
		10_000 * dollar(LIQUID),
		10_000 * dollar(STABLECOIN),
		false,
	)?;
	inject_liquidity(
		maker.clone(),
		STAKING,
		LIQUID,
		10_000 * dollar(STAKING),
		10_000 * dollar(LIQUID),
		false,
	)?;

	// purposly inject too little liquidity to have failed path, still reads dexs to check for viable
	// swap paths
	inject_liquidity(
		maker.clone(),
		STAKING,
		STABLECOIN,
		10 * dollar(STAKING),
		10 * dollar(STABLECOIN),
		false,
	)?;
	inject_liquidity(
		maker.clone(),
		LCDOT,
		STABLECOIN,
		dollar(LCDOT),
		dollar(STABLECOIN),
		false,
	)?;
	inject_liquidity(maker.clone(), LCDOT, STAKING, dollar(LCDOT), dollar(STAKING), false)?;

	// Add and initialize stable pools, is manually added with changes to runtime
	let assets_1 = vec![STAKING, LIQUID];
	create_stable_pools(assets_1.clone(), vec![1, 1], 10000u128);
	for asset in assets_1 {
		<Currencies as MultiCurrencyExtended<_>>::update_balance(asset, &maker, 1_000_000_000_000_000)?;
	}
	StableAsset::mint(
		RawOrigin::Signed(maker.clone()).into(),
		0,
		vec![1_000_000_000_000, 1_000_000_000_000],
		0,
	)?;

	// Adds `AggregatedSwapPaths`, also mirrors runtimes state
	AggregatedDex::update_aggregated_swap_paths(
		RawOrigin::Root.into(),
		vec![
			(
				(STAKING, STABLECOIN),
				Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LIQUID, STABLECOIN])]),
			),
			(
				(LIQUID, STABLECOIN),
				Some(vec![SwapPath::Taiga(0, 1, 0), SwapPath::Dex(vec![STAKING, STABLECOIN])]),
			),
		],
	)?;

	Ok(())
}

pub fn alice() -> AccountId {
	<Runtime as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr())
}
pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn erc20_address() -> EvmAddress {
	EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643").unwrap()
}

pub fn deploy_contract() {
	//let alice_account = alice_account_id();
	set_balance(NATIVE, &alice(), 1_000_000 * dollar(NATIVE));

	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	assert_ok!(EVM::create(
		RuntimeOrigin::signed(alice()),
		code,
		0,
		2_100_000,
		1_000_000,
		vec![]
	));
}
