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

use crate::{AccountId, AuctionId, AuctionManager, CdpTreasury, Currencies, EmergencyShutdown, Price, Runtime};

use super::utils::{dollar, feed_price, STABLECOIN, STAKING};
use frame_benchmarking::account;
use frame_system::RawOrigin;
use module_support::{AuctionManager as AuctionManagerTrait, CDPTreasury};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_runtime::FixedPointNumber;
use sp_std::vec;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_auction_manager }

	// `cancel` a collateral auction, worst case:
	// auction have been already bid
	cancel_collateral_auction {
		let bidder: AccountId = account("bidder", 0, SEED);
		let funder: AccountId = account("funder", 0, SEED);

		// set balance
		Currencies::deposit(STABLECOIN, &bidder, 80 * dollar(STABLECOIN))?;
		Currencies::deposit(STAKING, &funder, dollar(STAKING))?;
		CdpTreasury::deposit_collateral(&funder, STAKING, dollar(STAKING))?;

		// feed price
		feed_price(vec![(STAKING, Price::saturating_from_integer(120))])?;

		// create collateral auction
		AuctionManager::new_collateral_auction(&funder, STAKING, dollar(STAKING), 100 * dollar(STABLECOIN))?;
		let auction_id: AuctionId = Default::default();

		// bid collateral auction
		AuctionManager::collateral_auction_bid_handler(1, auction_id, (bidder, 80 * dollar(STABLECOIN)), None)?;

		// shutdown
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: cancel(RawOrigin::None, auction_id)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
