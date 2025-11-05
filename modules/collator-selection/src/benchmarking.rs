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
use frame_support::{
	assert_ok,
	pallet_prelude::Decode,
	traits::{Currency, EstimateNextSessionRotation, Get, Hooks},
};
use frame_system::RawOrigin;
use pallet_authorship::EventHandler;
use pallet_session::SessionManager;
use sp_std::{vec, vec::Vec};

/// Helper trait for benchmarking.
pub trait BenchmarkHelper<CurrencyId, Moment> {
	fn setup_on_initialize(n: u32, u: u32);
	fn setup_inject_liquidity() -> Option<(CurrencyId, CurrencyId, Moment)>;
}

impl<CurrencyId, Moment> BenchmarkHelper<CurrencyId, Moment> for () {
	fn setup_on_initialize(_n: u32, _u: u32) {}
	fn setup_inject_liquidity() -> Option<(CurrencyId, CurrencyId, Moment)> {
		None
	}
}

fn register_candidates<T>(count: u32)
where
	T: Config + pallet_session::Config,
{
	let candidates = (0..count).map(|c| account("candidate", c, 0)).collect::<Vec<_>>();
	assert!(CandidacyBond::<T>::get() > 0u32.into(), "Bond cannot be zero!");
	for (index, who) in candidates.iter().enumerate() {
		T::Currency::make_free_balance_be(&who, CandidacyBond::<T>::get() * 2u32.into());

		let mut keys = [1u8; 128];
		keys[0..4].copy_from_slice(&(index as u32).to_be_bytes());
		let keys: <T as pallet_session::Config>::Keys = Decode::decode(&mut &keys[..]).unwrap();
		assert_ok!(pallet_session::Pallet::<T>::set_keys(
			RawOrigin::Signed(who.clone()).into(),
			keys,
			vec![]
		));

		assert_ok!(Pallet::<T>::register_as_candidate(
			RawOrigin::Signed(who.clone()).into()
		));
	}
}

#[benchmarks(
	where
		T: Config + pallet_session::Config + pallet_authorship::Config,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn set_invulnerables(b: Liner<1, { T::MaxInvulnerables::get() }>) {
		let new_invulnerables = (0..b).map(|c| account("candidate", c, 0)).collect::<Vec<_>>();

		#[extrinsic_call]
		_(RawOrigin::Root, new_invulnerables.clone());

		frame_system::Pallet::<T>::assert_last_event(
			Event::NewInvulnerables {
				new_invulnerables: new_invulnerables,
			}
			.into(),
		);
	}

	#[benchmark]
	fn set_desired_candidates() {
		let max: u32 = T::MaxInvulnerables::get();

		#[extrinsic_call]
		_(RawOrigin::Root, max.clone());

		frame_system::Pallet::<T>::assert_last_event(
			Event::NewDesiredCandidates {
				new_desired_candidates: max,
			}
			.into(),
		);
	}

	#[benchmark]
	fn set_candidacy_bond() {
		let bond = T::Currency::minimum_balance() * 10u32.into();

		#[extrinsic_call]
		_(RawOrigin::Root, bond.clone());

		frame_system::Pallet::<T>::assert_last_event(
			Event::NewCandidacyBond {
				new_candidacy_bond: bond,
			}
			.into(),
		);
	}

	// worse case is when we have all the max-candidate slots filled except one, and we fill that
	// one.
	#[benchmark]
	fn register_as_candidate(c: Liner<{ T::MinCandidates::get() }, { T::MaxCandidates::get() }>) {
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);
		register_candidates::<T>(c - 1);

		let caller: T::AccountId = account("candidate", c, 0);
		let bond = T::Currency::minimum_balance() * 2u32.into();
		T::Currency::make_free_balance_be(&caller, bond.clone());

		let keys: <T as pallet_session::Config>::Keys = Decode::decode(&mut &[0u8; 128][..]).unwrap();
		pallet_session::Pallet::<T>::set_keys(RawOrigin::Signed(caller.clone()).into(), keys, vec![]).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		frame_system::Pallet::<T>::assert_last_event(
			Event::CandidateAdded {
				who: caller,
				bond: bond / 2u32.into(),
			}
			.into(),
		);
	}

	#[benchmark]
	fn register_candidate(c: Liner<1, { T::MaxCandidates::get() }>) {
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);
		register_candidates::<T>(c - 1);

		let caller: T::AccountId = account("candidate", c, 0);
		let bond = T::Currency::minimum_balance() * 2u32.into();
		T::Currency::make_free_balance_be(&caller, bond.clone());

		let keys: <T as pallet_session::Config>::Keys = Decode::decode(&mut &[0u8; 128][..]).unwrap();
		pallet_session::Pallet::<T>::set_keys(RawOrigin::Signed(caller.clone()).into(), keys, vec![]).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, caller.clone());

		frame_system::Pallet::<T>::assert_last_event(
			Event::CandidateAdded {
				who: caller,
				bond: 0u32.into(),
			}
			.into(),
		);
	}

	// worse case is the last candidate leaving.
	#[benchmark]
	fn leave_intent(c: Liner<{ T::MinCandidates::get() + 1 }, { T::MaxCandidates::get() }>) {
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);
		register_candidates::<T>(c);

		let leaving = Candidates::<T>::get().into_iter().last().unwrap();
		whitelist_account!(leaving);

		#[extrinsic_call]
		_(RawOrigin::Signed(leaving.clone()));

		frame_system::Pallet::<T>::assert_last_event(Event::CandidateRemoved { who: leaving }.into());
	}

	#[benchmark]
	fn withdraw_bond(c: Liner<{ T::MinCandidates::get() + 1 }, { T::MaxCandidates::get() }>) {
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);
		register_candidates::<T>(c);

		let leaving = Candidates::<T>::get().into_iter().last().unwrap();
		whitelist_account!(leaving);

		assert_ok!(Pallet::<T>::leave_intent(RawOrigin::Signed(leaving.clone()).into()));

		let session_duration = <T as pallet_session::Config>::NextSessionRotation::average_session_length();
		pallet_session::Pallet::<T>::on_initialize(session_duration);
		pallet_session::Pallet::<T>::on_initialize(session_duration * 2u32.into());

		#[extrinsic_call]
		_(RawOrigin::Signed(leaving));
	}

	// worse case is paying a non-existing candidate account.
	#[benchmark]
	fn note_author() {
		let c = T::MaxCandidates::get();
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);
		register_candidates::<T>(c);

		T::Currency::make_free_balance_be(&Pallet::<T>::account_id(), T::Currency::minimum_balance() * 2u32.into());
		let author = account("author", 0, 0);
		T::Currency::make_free_balance_be(&author, T::Currency::minimum_balance());
		assert!(T::Currency::free_balance(&author) == T::Currency::minimum_balance());

		#[block]
		{
			Pallet::<T>::note_author(author);
		}
	}

	// worse case is on new session.
	#[benchmark]
	fn new_session() {
		let c = T::MaxCandidates::get();
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);

		frame_system::Pallet::<T>::set_block_number(0u32.into());

		register_candidates::<T>(c);

		frame_system::Pallet::<T>::set_block_number(20u32.into());

		assert!(Candidates::<T>::get().len() == c as usize);

		#[block]
		{
			Pallet::<T>::new_session(0);
		}
	}

	#[benchmark]
	fn start_session(
		r: Liner<{ T::MinCandidates::get() }, { T::MaxCandidates::get() }>,
		c: Liner<{ T::MinCandidates::get() }, { T::MaxCandidates::get() }>,
	) {
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);

		frame_system::Pallet::<T>::set_block_number(0u32.into());
		register_candidates::<T>(c);

		frame_system::Pallet::<T>::set_block_number(20u32.into());

		let session_duration = <T as pallet_session::Config>::NextSessionRotation::average_session_length();
		pallet_session::Pallet::<T>::on_initialize(session_duration);
		pallet_session::Pallet::<T>::on_initialize(session_duration * 2u32.into());

		assert!(Candidates::<T>::get().len() == c as usize);

		#[block]
		{
			Pallet::<T>::start_session(2);
		}
	}

	#[benchmark]
	fn end_session(
		r: Liner<{ T::MinCandidates::get() }, { T::MaxCandidates::get() }>,
		c: Liner<{ T::MinCandidates::get() }, { T::MaxCandidates::get() }>,
	) {
		CandidacyBond::<T>::put(T::Currency::minimum_balance());
		DesiredCandidates::<T>::put(c);

		frame_system::Pallet::<T>::set_block_number(0u32.into());
		register_candidates::<T>(c);

		let candidates = Candidates::<T>::get();
		let removals = c
			.checked_sub(r)
			.unwrap_or_default()
			.checked_sub(T::MinCandidates::get())
			.unwrap_or_default();

		let mut count = 0;
		candidates.iter().for_each(|candidate| {
			if count < removals {
				// point = 0, will be removed.
				SessionPoints::<T>::insert(&candidate, 0);
			} else {
				SessionPoints::<T>::insert(
					&candidate,
					T::CollatorKickThreshold::get().mul_floor(1000 * POINT_PER_BLOCK),
				);
			}
			count += 1;
		});

		frame_system::Pallet::<T>::set_block_number(20u32.into());

		assert!(Candidates::<T>::get().len() == c as usize);

		#[block]
		{
			Pallet::<T>::end_session(0);
		}

		assert!(Candidates::<T>::get().len() == (c - removals) as usize);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
