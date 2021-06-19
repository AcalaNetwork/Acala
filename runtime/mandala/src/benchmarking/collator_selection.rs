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
	AccountId, Balance, Balances, CollatorSelection, Event, MaxCandidates, MaxInvulnerables, MinCandidates, Runtime,
	Session, SessionKeys, System,
};

use frame_benchmarking::{account, whitelisted_caller};
use frame_support::{assert_ok, pallet_prelude::Decode, traits::Currency};
use frame_system::RawOrigin;
use orml_benchmarking::{runtime_benchmarks, whitelist_account};
use pallet_authorship::EventHandler;
use pallet_session::SessionManager;
use sp_std::prelude::*;

const SEED: u32 = 0;

fn assert_last_event(generic_event: Event) {
	System::assert_last_event(generic_event.into());
}

fn register_candidates(count: u32) {
	let candidates = (0..count).map(|c| account("candidate", c, SEED)).collect::<Vec<_>>();
	assert!(
		module_collator_selection::CandidacyBond::<Runtime>::get() > 0u32.into(),
		"Bond cannot be zero!"
	);
	for (index, who) in candidates.iter().enumerate() {
		Balances::make_free_balance_be(
			&who,
			module_collator_selection::CandidacyBond::<Runtime>::get()
				.checked_mul(2u32.into())
				.unwrap(),
		);
		let mut keys = [1u8; 128];
		keys[0..8].copy_from_slice(&index.to_be_bytes());
		let keys: SessionKeys = Decode::decode(&mut &keys[..]).unwrap();
		Session::set_keys(RawOrigin::Signed(who.clone()).into(), keys, vec![]).unwrap();
		CollatorSelection::register_as_candidate(RawOrigin::Signed(who.clone()).into()).unwrap();
	}
}

runtime_benchmarks! {
	{ Runtime, module_collator_selection }

	set_invulnerables {
		let b in 1 .. MaxInvulnerables::get();
		let new_invulnerables = (0..b).map(|c| account("candidate", c, SEED)).collect::<Vec<_>>();
	}: {
		assert_ok!(
			CollatorSelection::set_invulnerables(RawOrigin::Root.into(), new_invulnerables.clone())
		);
	}
	verify {
		assert_last_event(module_collator_selection::Event::NewInvulnerables(new_invulnerables).into());
	}

	set_desired_candidates {
		let max: u32 = MaxInvulnerables::get();
	}: {
		assert_ok!(
			CollatorSelection::set_desired_candidates(RawOrigin::Root.into(), max.clone())
		);
	}
	verify {
		assert_last_event(module_collator_selection::Event::NewDesiredCandidates(max).into());
	}

	set_candidacy_bond {
		let bond: Balance = Balances::minimum_balance().checked_mul(10u32.into()).unwrap();
	}: {
		assert_ok!(
			CollatorSelection::set_candidacy_bond(RawOrigin::Root.into(), bond.clone())
		);
	}
	verify {
		assert_last_event(module_collator_selection::Event::NewCandidacyBond(bond).into());
	}

	// worse case is when we have all the max-candidate slots filled except one, and we fill that
	// one.
	register_as_candidate {
		let c in 1 .. MaxCandidates::get();

		module_collator_selection::CandidacyBond::<Runtime>::put(Balances::minimum_balance());
		module_collator_selection::DesiredCandidates::<Runtime>::put(c);
		register_candidates(c-1);

		let caller: AccountId = whitelisted_caller();
		let bond: Balance = Balances::minimum_balance().checked_mul(2u32.into()).unwrap();
		Balances::make_free_balance_be(&caller, bond.clone());

		Session::set_keys(RawOrigin::Signed(caller.clone()).into(), SessionKeys::default(), vec![]).unwrap();
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert_last_event(module_collator_selection::Event::CandidateAdded(caller, bond.checked_div(2u32.into()).unwrap()).into());
	}

	register_candidate {
		let c in 1 .. MaxCandidates::get();

		module_collator_selection::CandidacyBond::<Runtime>::put(Balances::minimum_balance());
		module_collator_selection::DesiredCandidates::<Runtime>::put(c);
		register_candidates(c-1);

		let caller: AccountId = whitelisted_caller();
		let bond: Balance = Balances::minimum_balance().checked_mul(2u32.into()).unwrap();
		Balances::make_free_balance_be(&caller, bond.clone());

		Session::set_keys(RawOrigin::Signed(caller.clone()).into(), SessionKeys::default(), vec![]).unwrap();
	}: _(RawOrigin::Root, caller.clone())
	verify {
		assert_last_event(module_collator_selection::Event::CandidateAdded(caller, 0).into());
	}

	// worse case is the last candidate leaving.
	leave_intent {
		// MinCandidates = 5, so begin with 6.
		let c in 6 .. MaxCandidates::get();
		module_collator_selection::CandidacyBond::<Runtime>::put(Balances::minimum_balance());
		module_collator_selection::DesiredCandidates::<Runtime>::put(c);
		register_candidates(c);

		let leaving = module_collator_selection::Candidates::<Runtime>::get().into_iter().last().unwrap();
		whitelist_account!(leaving);
	}: _(RawOrigin::Signed(leaving.clone()))
	verify {
		assert_last_event(module_collator_selection::Event::CandidateRemoved(leaving).into());
	}

	// worse case is paying a non-existing candidate account.
	note_author {
		let c = MaxCandidates::get();
		module_collator_selection::CandidacyBond::<Runtime>::put(Balances::minimum_balance());
		module_collator_selection::DesiredCandidates::<Runtime>::put(c);
		register_candidates(c);

		Balances::make_free_balance_be(
			&CollatorSelection::account_id(),
			Balances::minimum_balance().checked_mul(2u32.into()).unwrap()
		);
		let author = account("author", 0, SEED);
		assert!(Balances::free_balance(&author) == 0u32.into());
	}: {
		CollatorSelection::note_author(author.clone())
	}

	// worse case is on new session.
	new_session {
		let c = MaxCandidates::get();
		module_collator_selection::CandidacyBond::<Runtime>::put(Balances::minimum_balance());
		module_collator_selection::DesiredCandidates::<Runtime>::put(c);
		System::set_block_number(0u32.into());
		register_candidates(c);

		System::set_block_number(20u32.into());

		assert!(module_collator_selection::Candidates::<Runtime>::get().len() == c as usize);
	}: {
		CollatorSelection::new_session(0)
	}

	start_session {
		// MinCandidates = 5, so begin with 5.
		let r in 5 .. MaxCandidates::get();
		let c in 5 .. MaxCandidates::get();

		module_collator_selection::CandidacyBond::<Runtime>::put(Balances::minimum_balance());
		module_collator_selection::DesiredCandidates::<Runtime>::put(c);
		System::set_block_number(0u32.into());
		register_candidates(c);

		// TODO: https://github.com/paritytech/substrate/pull/8815
		// for i in 0..r {
		//     pallet_session::Validators::insert()
		// }
		System::set_block_number(20u32.into());

		assert!(module_collator_selection::Candidates::<Runtime>::get().len() == c as usize);
	}: {
		CollatorSelection::start_session(2)
	}

	end_session {
		// MinCandidates = 5, so begin with 5.
		let r in 5 .. MaxCandidates::get();
		let c in 5 .. MaxCandidates::get();

		module_collator_selection::CandidacyBond::<Runtime>::put(Balances::minimum_balance());
		module_collator_selection::DesiredCandidates::<Runtime>::put(c);
		System::set_block_number(0u32.into());
		register_candidates(c);

		let candidates = module_collator_selection::Candidates::<Runtime>::get();
		let removals = c.checked_sub(r).unwrap_or_default().checked_sub(MinCandidates::get()).unwrap_or_default();

		let mut count = 0;
		candidates.iter().for_each(|candidate| {
			if count < removals {
				// point = 0, will be removed.
				module_collator_selection::SessionPoints::<Runtime>::insert(&candidate, 0);
			} else {
				module_collator_selection::SessionPoints::<Runtime>::insert(&candidate, 1);
			}
			count += 1;
		});

		System::set_block_number(20u32.into());

		assert!(module_collator_selection::Candidates::<Runtime>::get().len() == c as usize);
	}: {
		CollatorSelection::end_session(0)
	} verify {
		assert!(module_collator_selection::Candidates::<Runtime>::get().len() == (c - removals) as usize);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
