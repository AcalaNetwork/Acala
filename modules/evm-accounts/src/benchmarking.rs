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
use frame_system::RawOrigin;

fn alice() -> libsecp256k1::SecretKey {
	libsecp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[benchmarks(
where
	T::AccountId: From<[u8; 32]>,
	BalanceOf<T>: From<u128>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn claim_account() {
		let caller: T::AccountId = account("caller", 0, 0);

		let address = Pallet::<T>::eth_address(&alice());
		let mut data = [0u8; 32];
		data[0..4].copy_from_slice(b"evm:");
		data[4..24].copy_from_slice(&address[..]);
		let alice_account_id = T::AccountId::from(data);

		let amount: BalanceOf<T> = 1_000_000_000_000_000_000u128.into();

		let _ = T::Currency::make_free_balance_be(&alice_account_id, amount);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller.clone()),
			Pallet::<T>::eth_address(&alice()),
			Pallet::<T>::eth_sign(&alice(), &caller),
		);

		frame_system::Pallet::<T>::assert_last_event(
			Event::ClaimAccount {
				account_id: caller.clone(),
				evm_address: Pallet::<T>::eth_address(&alice()),
			}
			.into(),
		);

		assert_eq!(T::Currency::free_balance(&caller), amount);
	}

	#[benchmark]
	fn claim_default_account() {
		let caller = account("caller", 0, 0);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
