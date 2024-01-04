// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use crate::{AccountId, Auction, AuctionId, AuctionManager, AuctionTimeToClose, CdpTreasury, Runtime, System};

use super::utils::{dollar, set_balance, STABLECOIN, STAKING};
use frame_benchmarking::{account, whitelisted_caller};
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

	// `bid` a collateral auction, best cases:
	// there's no bidder before and bid price doesn't exceed target amount
	#[extra]
	bid_collateral_auction_as_first_bidder {
		let d in 1 .. MAX_DOLLARS;

		let bidder: AccountId = whitelisted_caller();
		let funder = account("funder", 0, SEED);
		let collateral_amount = 100 * dollar(STAKING);
		let target_amount = 10_000 * dollar(STABLECOIN);
		let bid_price = (5_000u128 + d as u128) * dollar(STABLECOIN);
		let auction_id: AuctionId = 0;

		set_balance(STAKING, &funder, collateral_amount);
		set_balance(STABLECOIN, &bidder, bid_price);
		<CdpTreasury as CDPTreasury<_>>::deposit_collateral(&funder, STAKING, collateral_amount)?;
		AuctionManager::new_collateral_auction(&funder, STAKING, collateral_amount, target_amount)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a collateral auction, worst cases:
	// there's bidder before and bid price will exceed target amount
	bid_collateral_auction {
		let bidder: AccountId = whitelisted_caller();
		let previous_bidder = account("previous_bidder", 0, SEED);
		let funder = account("funder", 0, SEED);
		let collateral_amount = 100 * dollar(STAKING);
		let target_amount = 10_000 * dollar(STABLECOIN);
		let previous_bid_price = 5_000u128 * dollar(STABLECOIN);
		let bid_price = 10_000u128 * dollar(STABLECOIN);
		let auction_id: AuctionId = 0;

		set_balance(STAKING, &funder, collateral_amount);
		set_balance(STABLECOIN, &bidder, bid_price);
		set_balance(STABLECOIN, &previous_bidder, previous_bid_price);
		<CdpTreasury as CDPTreasury<_>>::deposit_collateral(&funder, STAKING, collateral_amount)?;
		AuctionManager::new_collateral_auction(&funder, STAKING, collateral_amount, target_amount)?;
		Auction::bid(RawOrigin::Signed(previous_bidder).into(), auction_id, previous_bid_price)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	on_finalize {
		let c in 1 .. MAX_AUCTION_ID;

		let bidder = account("bidder", 0, SEED);
		let funder = account("funder", 0, SEED);
		let collateral_amount = 100 * dollar(STAKING);
		let target_amount = 10_000 * dollar(STABLECOIN);
		let bid_price = 5_000u128 * dollar(STABLECOIN);

		System::set_block_number(1);
		for auction_id in 0 .. c {
			set_balance(STAKING, &funder, collateral_amount);
			<CdpTreasury as CDPTreasury<_>>::deposit_collateral(&funder, STAKING, collateral_amount)?;
			AuctionManager::new_collateral_auction(&funder, STAKING, collateral_amount, target_amount)?;
			set_balance(STABLECOIN, &bidder, bid_price);
			Auction::bid(RawOrigin::Signed(bidder.clone()).into(), auction_id, bid_price)?;
		}
	}: {
		Auction::on_finalize(System::block_number() + AuctionTimeToClose::get());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
