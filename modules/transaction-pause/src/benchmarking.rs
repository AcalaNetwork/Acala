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

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn pause_transaction() {
		#[extrinsic_call]
		_(RawOrigin::Root, b"Balances".to_vec(), b"transfer".to_vec());
	}

	#[benchmark]
	fn unpause_transaction() {
		assert_ok!(Pallet::<T>::pause_transaction(
			RawOrigin::Root.into(),
			b"Balances".to_vec(),
			b"transfer".to_vec()
		));

		#[extrinsic_call]
		_(RawOrigin::Root, b"Balances".to_vec(), b"transfer".to_vec());
	}

	#[benchmark]
	fn pause_evm_precompile() {
		#[extrinsic_call]
		_(RawOrigin::Root, H160::from_low_u64_be(1));
	}

	#[benchmark]
	fn unpause_evm_precompile() {
		assert_ok!(Pallet::<T>::pause_evm_precompile(
			RawOrigin::Root.into(),
			H160::from_low_u64_be(1)
		));

		#[extrinsic_call]
		_(RawOrigin::Root, H160::from_low_u64_be(1));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
