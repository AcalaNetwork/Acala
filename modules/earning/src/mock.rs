// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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
	construct_runtime, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Imbalance},
};
use pallet_balances::NegativeImbalance;
use primitives::mock_handler;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};

pub type AccountId = u128;
pub type BlockNumber = u64;

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = Everything;
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<10>;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

parameter_types! {
	pub const InstantUnstakeFee: Permill = Permill::from_percent(10);
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

impl Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type OnBonded = OnBonded;
	type OnUnbonded = OnUnbonded;
	type OnUnstakeFee = OnUnstakeFee;
	type MinBond = ConstU128<100>;
	type UnbondingPeriod = ConstU64<3>;
	type InstantUnstakeFee = InstantUnstakeFee;
	type MaxUnbondingChunks = ConstU32<3>;
	type LockIdentifier = EarningLockIdentifier;
	type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
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
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
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
