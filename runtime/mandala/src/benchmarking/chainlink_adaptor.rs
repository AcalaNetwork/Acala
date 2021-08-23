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

use crate::{dollar, AccountId, ChainlinkAdaptor, ChainlinkFeed, GetNativeCurrencyId, Runtime};

use super::utils::set_balance;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::traits::Bounded;
use sp_std::vec;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, ecosystem_chainlink_adaptor }

	map_feed_id {
		let who: AccountId = account("who", 0, SEED);
		let currency_id = GetNativeCurrencyId::get();
		set_balance(currency_id, &who, dollar(currency_id) * 100);
		ChainlinkAdaptor::overwrite_chainlink_feed_admin(
			RawOrigin::Root.into(),
			who.clone()
		)?;
		ChainlinkFeed::set_feed_creator(
			RawOrigin::Signed(who.clone()).into(),
			who.clone()
		)?;
		ChainlinkFeed::create_feed(
			RawOrigin::Signed(who.clone()).into(),
			20,
			10,
			(Bounded::min_value(), Bounded::max_value()),
			1,
			0,
			b"nativeusd".to_vec(),
			0,
			vec![(who.clone(), who)],
			None,
			None,
		)?;
	}: _(RawOrigin::Root, 0, currency_id)

	unmap_feed_id {
		let who: AccountId = account("who", 0, SEED);
		let currency_id = GetNativeCurrencyId::get();
		set_balance(currency_id, &who, dollar(currency_id) * 100);
		ChainlinkAdaptor::overwrite_chainlink_feed_admin(
			RawOrigin::Root.into(),
			who.clone()
		)?;
		ChainlinkFeed::set_feed_creator(
			RawOrigin::Signed(who.clone()).into(),
			who.clone()
		)?;
		ChainlinkFeed::create_feed(
			RawOrigin::Signed(who.clone()).into(),
			20,
			10,
			(Bounded::min_value(), Bounded::max_value()),
			1,
			0,
			b"nativeusd".to_vec(),
			0,
			vec![(who.clone(), who)],
			None,
			None,
		)?;
		ChainlinkAdaptor::map_feed_id(
			RawOrigin::Root.into(),
			0,
			currency_id
		)?;
	}: _(RawOrigin::Root, currency_id)

	overwrite_chainlink_feed_admin {
		let who: AccountId = account("who", 0, SEED);
	}: _(RawOrigin::Root, who)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
