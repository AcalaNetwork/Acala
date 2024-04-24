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

//! Mocks for the prices module.

#![cfg(test)]

use super::*;
use crate as earning;
use frame_support::{
	construct_runtime, derive_impl, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Imbalance},
};
use pallet_balances::NegativeImbalance;
use primitives::mock_handler;
use sp_runtime::{traits::IdentityLookup, BuildStorage};

pub type AccountId = u128;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<10>;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}

parameter_types! {
	pub const EarningLockIdentifier: LockIdentifier = *b"12345678";
}

mock_handler! {
	pub struct OnBonded<(AccountId, Balance)>;
	pub struct OnUnbonded<(AccountId, Balance)>;
	pub struct OnUnstakeFee<Balance>;
}

impl OnUnbalanced<NegativeImbalance<Runtime>> for OnUnstakeFee {
	fn on_nonzero_unbalanced(amount: NegativeImbalance<Runtime>) {
		Self::push(amount.peek());
	}
}

pub struct ParameterStoreImpl;
impl ParameterStore<Parameters> for ParameterStoreImpl {
	fn get<K>(key: K) -> Option<K::Value>
	where
		K: orml_traits::parameters::Key
			+ Into<<Parameters as orml_traits::parameters::AggregratedKeyValue>::AggregratedKey>,
		<Parameters as orml_traits::parameters::AggregratedKeyValue>::AggregratedValue: TryInto<K::WrappedValue>,
	{
		let key = key.into();
		match key {
			ParametersKey::InstantUnstakeFee(_) => Some(
				ParametersValue::InstantUnstakeFee(Permill::from_percent(10))
					.try_into()
					.ok()?
					.into(),
			),
		}
	}
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ParameterStore = ParameterStoreImpl;
	type OnBonded = OnBonded;
	type OnUnbonded = OnUnbonded;
	type OnUnstakeFee = OnUnstakeFee;
	type MinBond = ConstU128<100>;
	type UnbondingPeriod = ConstU64<3>;
	type MaxUnbondingChunks = ConstU32<3>;
	type LockIdentifier = EarningLockIdentifier;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		Earning: earning,
	}
);

pub struct ExtBuilder;

pub const ALICE: AccountId = 1;

impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![(ALICE, 1000)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut t: sp_io::TestExternalities = t.into();

		t.execute_with(|| {
			System::set_block_number(1);
		});

		t
	}
}
