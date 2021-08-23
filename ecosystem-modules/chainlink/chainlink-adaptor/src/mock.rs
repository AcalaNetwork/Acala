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

//! Mocks for the chainlink adaptor module.

#![cfg(test)]

use super::*;
use frame_support::{construct_runtime, ord_parameter_types, parameter_types, PalletId};
use frame_system::EnsureSignedBy;
use primitives::{Balance, FeedId, Moment, TokenSymbol};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};

pub type BlockNumber = u64;
pub type AccountId = u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const KSM: CurrencyId = CurrencyId::Token(TokenSymbol::KSM);

mod chainlink_adaptor {
	pub use super::super::*;
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
}

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
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
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
	type AccountStore = System;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type MaxLocks = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: Moment = 1000;
}

impl pallet_timestamp::Config for Runtime {
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const FeedPalletId: PalletId = PalletId(*b"linkfeed");
	pub const StringLimit: u32 = 15;
	pub const OracleLimit: u32 = 10;
	pub const FeedLimit: FeedId = 10;
}

impl pallet_chainlink_feed::Config for Runtime {
	type Event = Event;
	type FeedId = FeedId;
	type Value = u128;
	type Currency = Balances;
	type PalletId = FeedPalletId;
	type MinimumReserve = ExistentialDeposit;
	type StringLimit = StringLimit;
	type OnAnswerHandler = ChainlinkAdaptor;
	type OracleCountLimit = OracleLimit;
	type FeedLimit = FeedLimit;
	type WeightInfo = ();
}

pub struct MockConvert;
impl Convert<u128, Option<Price>> for MockConvert {
	fn convert(value: u128) -> Option<Price> {
		Some(Price::from_inner(value))
	}
}

ord_parameter_types! {
	pub const RegistorOrigin: AccountId = 11;
}

impl Config for Runtime {
	type Event = Event;
	type Convert = MockConvert;
	type Time = Timestamp;
	type RegistorOrigin = EnsureSignedBy<RegistorOrigin, AccountId>;
	type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Event<T>, Config<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		ChainlinkFeed: pallet_chainlink_feed::{Pallet, Call, Storage, Event<T>, Config<T>},
		ChainlinkAdaptor: chainlink_adaptor::{Pallet, Call, Storage, Event<T>},
	}
);

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, 1_000), (BOB, 1_000)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_chainlink_feed::GenesisConfig::<Runtime> {
			pallet_admin: None,
			feed_creators: vec![ALICE],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
