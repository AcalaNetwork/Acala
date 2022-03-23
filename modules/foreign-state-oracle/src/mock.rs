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

//! Mock for foreign state oracle

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{Everything, IsType},
};
use frame_system::EnsureSignedBy;
use primitives::ReserveIdentifier;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

// example module using foreign state query
#[frame_support::pallet]
pub mod query_example {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The overarching call type.
		type Call: Parameter
			+ Dispatchable<Origin = Self::Origin>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>
			+ From<Call<Self>>
			+ IsType<<Self as frame_system::Config>::Call>;

		type ForeignStateQuery: ForeignChainStateQuery<
			Self::AccountId,
			<Self as Config>::Call,
			Self::BlockNumber,
			Self::Origin,
		>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		OriginInjected { origin_data: Vec<u8>, call_data: Vec<u8> },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn injected_call(origin: OriginFor<T>, call_data: Vec<u8>) -> DispatchResult {
			let origin_data = T::ForeignStateQuery::ensure_origin(origin)?;
			Self::deposit_event(Event::<T>::OriginInjected { origin_data, call_data });
			Ok(())
		}

		#[transactional]
		#[pallet::weight(0)]
		pub fn mock_create_query(
			origin: OriginFor<T>,
			call_data: Vec<u8>,
			duration: Option<T::BlockNumber>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			T::ForeignStateQuery::create_query(&who, Call::<T>::injected_call { call_data }.into(), duration)
		}
		#[transactional]
		#[pallet::weight(0)]
		pub fn mock_cancel_query(_origin: OriginFor<T>, who: T::AccountId, index: QueryIndex) -> DispatchResult {
			T::ForeignStateQuery::cancel_query(&who, index)
		}
	}
}

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub type AccountId = u128;
pub type BlockNumber = u64;

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
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

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
}

impl query_example::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type ForeignStateQuery = ForeignStateOracle;
}

parameter_types! {
	pub const ForeignOraclePalletId: PalletId = PalletId(*b"aca/fsto");
	pub const QueryFee: Balance = 100;
	pub const CancelFee: Balance = 10;
	pub ExpiredCallPurgeReward: Permill = Permill::from_percent(50);
	pub const MaxQueryCallSize: u32 = 200;
}

impl Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type DispatchableCall = Call;
	type QueryFee = QueryFee;
	type CancelFee = CancelFee;
	type ExpiredCallPurgeReward = ExpiredCallPurgeReward;
	type MaxQueryCallSize = MaxQueryCallSize;
	type OracleOrigin = EnsureSignedBy<One, AccountId>;
	type Currency = Balances;
	type PalletId = ForeignOraclePalletId;
	type BlockNumberProvider = System;
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
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		ForeignStateOracle: module::{Pallet, Call, Storage, Event<T>, Origin},
		QueryExample: query_example::{Pallet, Call, Event<T>},
	}
);

pub struct ExtBuilder {
	endowed_native: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_native: vec![(ALICE, 1_000_000)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self.endowed_native,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
