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

//! Mocks for session-manager module.

#![cfg(test)]

use crate as session_manager;
use frame_support::{construct_runtime, derive_impl};
use sp_runtime::{testing::UintAuthorityId, traits::OpaqueKeys, BuildStorage, RuntimeAppPublic};

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = u64;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type Block = Block;
}

sp_runtime::impl_opaque_keys! {
	pub struct MockSessionKeys {
		// a key for aura authoring
		pub aura: UintAuthorityId,
	}
}

impl From<UintAuthorityId> for MockSessionKeys {
	fn from(aura: sp_runtime::testing::UintAuthorityId) -> Self {
		Self { aura }
	}
}

pub struct IdentityCollator;
impl<T> sp_runtime::traits::Convert<T, Option<T>> for IdentityCollator {
	fn convert(t: T) -> Option<T> {
		Some(t)
	}
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<u64> for TestSessionHandler {
	const KEY_TYPE_IDS: &'static [sp_runtime::KeyTypeId] = &[UintAuthorityId::ID];
	fn on_genesis_session<T: OpaqueKeys>(_validators: &[(u64, T)]) {}
	fn on_new_session<T: OpaqueKeys>(_changed: bool, _validators: &[(u64, T)], _queued_validators: &[(u64, T)]) {}
	fn on_disabled(_validator_index: u32) {}
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = IdentityCollator;
	type ShouldEndSession = SessionManager;
	type NextSessionRotation = SessionManager;
	type SessionManager = ();
	type SessionHandler = TestSessionHandler;
	type Keys = MockSessionKeys;
	type WeightInfo = ();
}

impl session_manager::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorSet = Session;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Session: pallet_session,
		SessionManager: session_manager,
	}
);

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap();
	session_manager::GenesisConfig::<Runtime> { session_duration: 10 }
		.assimilate_storage(&mut t)
		.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
