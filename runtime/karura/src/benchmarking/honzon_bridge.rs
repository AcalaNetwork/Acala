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

use crate::{AccountId, EvmAddress, HonzonBridge, Runtime};

use frame_benchmarking::account;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, module_honzon_bridge }
	set_bridged_stable_coin_address {
	}: _(RawOrigin::Root, EvmAddress::default())

	to_bridged {
		HonzonBridge::set_bridged_stable_coin_address(RawOrigin::Root.into(), EvmAddress::default())?;

		let caller: AccountId = account("caller", 0, 0);
	}: _(RawOrigin::Signed(caller), 0)

	from_bridged {
		HonzonBridge::set_bridged_stable_coin_address(RawOrigin::Root.into(), EvmAddress::default())?;

		let caller: AccountId = account("caller", 0, 0);
	}: _(RawOrigin::Signed(caller), 0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
