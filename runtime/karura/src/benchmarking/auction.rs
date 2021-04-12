// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

use crate::{
	dollar, Auction, AuctionId, AuctionManager, AuctionTimeToClose, CdpTreasury, Runtime, System, KAR, KSM, KUSD,
};

use super::utils::set_balance;
use frame_benchmarking::account;
use frame_support::traits::OnFinalize;
use frame_system::RawOrigin;
use module_support::{AuctionManager as AuctionManagerTrait, CDPTreasury};
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

const SEED: u32 = 0;
const MAX_DOLLARS: u32 = 1000;
const MAX_AUCTION_ID: u32 = 100;

runtime_benchmarks! {
	{ Runtime, orml_auction }

	_ {
		let d in 1 .. MAX_DOLLARS => ();
		let c in 1 .. MAX_AUCTION_ID => ();
	}

	// `bid` a collateral auction, best cases:
	// there's no bidder before and bid price doesn't exceed target amount
	#[extra]
	bid_collateral_auction_as_first_bidder {
		let bidder = account("bidder", 0, SEED);
		let funder = account("funder", 0, SEED);
		let currency_id = KSM;
		let collateral_amount = 100 * dollar(currency_id);
		let target_amount = 10_000 * dollar(KUSD);
		let bid_price = (5_000u128 + d as u128) * dollar(KUSD);
		let auction_id: AuctionId = 0;

		set_balance(currency_id, &funder, collateral_amount);
		set_balance(KUSD, &bidder, bid_price);
		<CdpTreasury as CDPTreasury<_>>::deposit_collateral(&funder, currency_id, collateral_amount)?;
		AuctionManager::new_collateral_auction(&funder, currency_id, collateral_amount, target_amount)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a collateral auction, worst cases:
	// there's bidder before and bid price will exceed target amount
	bid_collateral_auction {
		let bidder = account("bidder", 0, SEED);
		let previous_bidder = account("previous_bidder", 0, SEED);
		let funder = account("funder", 0, SEED);
		let currency_id = KSM;
		let collateral_amount = 100 * dollar(currency_id);
		let target_amount = 10_000 * dollar(KUSD);
		let previous_bid_price = (5_000u128 + d as u128) * dollar(KUSD);
		let bid_price = (10_000u128 + d as u128) * dollar(KUSD);
		let auction_id: AuctionId = 0;

		set_balance(currency_id, &funder, collateral_amount);
		set_balance(KUSD, &bidder, bid_price);
		set_balance(KUSD, &previous_bidder, previous_bid_price);
		<CdpTreasury as CDPTreasury<_>>::deposit_collateral(&funder, currency_id, collateral_amount)?;
		AuctionManager::new_collateral_auction(&funder, currency_id, collateral_amount, target_amount)?;
		Auction::bid(RawOrigin::Signed(previous_bidder).into(), auction_id, previous_bid_price)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a surplus auction, best cases:
	// there's no bidder before
	#[extra]
	bid_surplus_auction_as_first_bidder {
		let bidder = account("bidder", 0, SEED);

		let surplus_amount = 100 * dollar(KUSD);
		let bid_price = d * dollar(KAR);
		let auction_id: AuctionId = 0;

		set_balance(KAR, &bidder, bid_price);
		AuctionManager::new_surplus_auction(surplus_amount)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a surplus auction, worst cases:
	// there's bidder before
	bid_surplus_auction {
		let bidder = account("bidder", 0, SEED);
		let previous_bidder = account("previous_bidder", 0, SEED);
		let surplus_amount = 100 * dollar(KUSD);
		let bid_price = (d as u128 * 2u128) * dollar(KAR);
		let previous_bid_price = d * dollar(KAR);
		let auction_id: AuctionId = 0;

		set_balance(KAR, &bidder, bid_price);
		set_balance(KAR, &previous_bidder, previous_bid_price);
		AuctionManager::new_surplus_auction(surplus_amount)?;
		Auction::bid(RawOrigin::Signed(previous_bidder).into(), auction_id, previous_bid_price)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a debit auction, best cases:
	// there's no bidder before and bid price happens to be debit amount
	#[extra]
	bid_debit_auction_as_first_bidder {
		let bidder = account("bidder", 0, SEED);

		let fix_debit_amount = 100 * dollar(KUSD);
		let initial_amount = 10 * dollar(KAR);
		let auction_id: AuctionId = 0;

		set_balance(KUSD, &bidder, fix_debit_amount);
		AuctionManager::new_debit_auction(initial_amount ,fix_debit_amount)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, fix_debit_amount)

	// `bid` a debit auction, worst cases:
	// there's bidder before
	bid_debit_auction {
		let bidder = account("bidder", 0, SEED);
		let previous_bidder = account("previous_bidder", 0, SEED);
		let fix_debit_amount = 100 * dollar(KUSD);
		let initial_amount = 10 * dollar(KAR);
		let previous_bid_price = fix_debit_amount;
		let bid_price = fix_debit_amount * 2;
		let auction_id: AuctionId = 0;

		set_balance(KUSD, &bidder, bid_price);
		set_balance(KUSD, &previous_bidder, previous_bid_price);
		AuctionManager::new_debit_auction(initial_amount ,fix_debit_amount)?;
		Auction::bid(RawOrigin::Signed(previous_bidder).into(), auction_id, previous_bid_price)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	on_finalize {
		let c in ...;

		let bidder = account("bidder", 0, SEED);
		let fix_debit_amount = 100 * dollar(KUSD);
		let initial_amount = 10 * dollar(KAR);
		let auction_id: AuctionId = 0;
		set_balance(KUSD, &bidder, fix_debit_amount * c as u128);

		System::set_block_number(1);
		for auction_id in 0 .. c {
			AuctionManager::new_debit_auction(initial_amount ,fix_debit_amount)?;
			Auction::bid(RawOrigin::Signed(bidder.clone()).into(), auction_id, fix_debit_amount)?;
		}
	}: {
		Auction::on_finalize(System::block_number() + AuctionTimeToClose::get());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn bid_collateral_auction_as_first_bidder() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_collateral_auction_as_first_bidder());
		});
	}

	#[test]
	fn bid_collateral_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_collateral_auction());
		});
	}

	#[test]
	fn bid_surplus_auction_as_first_bidder() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_surplus_auction_as_first_bidder());
		});
	}

	#[test]
	fn bid_surplus_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_surplus_auction());
		});
	}

	#[test]
	fn bid_debit_auction_as_first_bidder() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_debit_auction_as_first_bidder());
		});
	}

	#[test]
	fn bid_debit_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_debit_auction());
		});
	}

	#[test]
	fn on_finalize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize());
		});
	}
}
