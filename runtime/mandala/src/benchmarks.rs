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
	dollar, AccountId, Auction, AuctionId, AuctionManager, AuctionTimeToClose, Balance, CdpTreasury, Currencies,
	CurrencyId, GetLiquidCurrencyId, GetNativeCurrencyId, GetStableCurrencyId, GetStakingCurrencyId, Price, RawOrigin,
	System, TreasuryPalletId,
};
use frame_benchmarking::account;
use frame_support::assert_ok;
use frame_system::pallet_prelude::BlockNumberFor;
use module_support::{AuctionManager as AuctionManagerTrait, CDPTreasury};
use orml_traits::MultiCurrencyExtended;
use primitives::AuthoritysOriginId;
use sp_runtime::{traits::AccountIdConversion, FixedPointNumber, FixedU128, SaturatedConversion};
use sp_std::vec;

pub const NATIVE: CurrencyId = GetNativeCurrencyId::get();
pub const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
pub const LIQUID: CurrencyId = GetLiquidCurrencyId::get();
pub const STAKING: CurrencyId = GetStakingCurrencyId::get();

pub fn set_balance(currency_id: CurrencyId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

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
		Some((TreasuryPalletId::get().into_account_truncating(), dollar(NATIVE)))
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
