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
use frame_system::{EventRecord, RawOrigin};

type BalanceOf<T> = <<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

fn assert_last_event<T: Config>(generic_event: <T as frame_system::Config>::RuntimeEvent) {
	let events = frame_system::Pallet::<T>::events();
	let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
	// compare to the last event record
	let EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn redeem() {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount: BalanceOf<T> = 1_000_000_000_000_000_000u128.into();

		assert_ok!(<T::Currency as MultiCurrency<_>>::deposit(
			T::LiquidCrowdloanCurrencyId::get(),
			&caller,
			amount
		));
		assert_ok!(<T::Currency as MultiCurrency<_>>::deposit(
			T::RelayChainCurrencyId::get(),
			&Pallet::<T>::account_id(),
			amount
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), amount);

		assert_last_event::<T>(
			Event::<T>::Redeemed {
				currency_id: T::RelayChainCurrencyId::get(),
				amount,
			}
			.into(),
		);
	}

	#[benchmark]
	fn set_redeem_currency_id() {
		#[extrinsic_call]
		_(RawOrigin::Root, T::RelayChainCurrencyId::get());

		assert_last_event::<T>(
			Event::<T>::RedeemCurrencyIdUpdated {
				currency_id: T::RelayChainCurrencyId::get(),
			}
			.into(),
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
