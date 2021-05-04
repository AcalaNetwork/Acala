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
	dollar, AcalaOracle, AccountId, AuctionId, AuctionManager, CdpTreasury, Currencies, EmergencyShutdown,
	GetStableCurrencyId, Price, Runtime, DOT,
};

use frame_benchmarking::account;
use frame_system::RawOrigin;
use module_support::AuctionManager as AuctionManagerTrait;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_runtime::FixedPointNumber;
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_auction_manager }

	// `cancel` a collateral auction, worst case:
	// auction have been already bid
	cancel_collateral_auction {
		let bidder: AccountId = account("bidder", 0, SEED);
		let funder: AccountId = account("funder", 0, SEED);
		let stable_currency_id = GetStableCurrencyId::get();

		// set balance
		Currencies::deposit(stable_currency_id, &bidder, 80 * dollar(stable_currency_id))?;
		Currencies::deposit(DOT, &funder, dollar(DOT))?;
		CdpTreasury::deposit_collateral(&funder, DOT, dollar(DOT))?;

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(DOT, Price::saturating_from_integer(120))])?;

		// create collateral auction
		AuctionManager::new_collateral_auction(&funder, DOT, dollar(DOT), 100 * dollar(stable_currency_id))?;
		let auction_id: AuctionId = Default::default();

		// bid collateral auction
		let _ = AuctionManager::collateral_auction_bid_handler(1, auction_id, (bidder, 80 * dollar(stable_currency_id)), None);

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
