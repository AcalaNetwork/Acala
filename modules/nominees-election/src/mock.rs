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

//! Mocks for nominees election module.

#![cfg(test)]

use super::*;

use crate as nominees_election;
use frame_support::{construct_runtime, parameter_types};
use orml_traits::parameter_type_with_key;
use primitives::{Amount, CurrencyId, TokenSymbol};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
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

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

parameter_types! {
	pub const MaxLocks: u32 = 100;
}

impl orml_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = MaxLocks;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
}

pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;
pub type LDOTCurrency = orml_currencies::Currency<Runtime, GetLiquidCurrencyId>;

impl orml_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = TokensModule;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinBondThreshold: Balance = 5;
	pub const BondingDuration: EraIndex = 4;
	pub const NominateesCount: u32 = 5;
	pub const MaxUnlockingChunks: u32 = 3;
}

pub struct MockRelaychainValidatorFilter;
impl Contains<AccountId> for MockRelaychainValidatorFilter {
	fn contains(a: &AccountId) -> bool {
		match a {
			0..=6 => true,
			_ => false,
		}
	}
}

impl Config for Runtime {
	type Event = Event;
	type Currency = LDOTCurrency;
	type NomineeId = AccountId;
	type MinBondThreshold = MinBondThreshold;
	type BondingDuration = BondingDuration;
	type NominateesCount = NominateesCount;
	type MaxUnlockingChunks = MaxUnlockingChunks;
	type RelaychainValidatorFilter = MockRelaychainValidatorFilter;
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
		NomineesElectionModule: nominees_election::{Pallet, Call, Storage, Event<T>},
		TokensModule: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		PalletBalances: pallet_balances::{Pallet, Call, Storage, Event<T>},
		OrmlCurrencies: orml_currencies::{Pallet, Call, Event<T>},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![(ALICE, LDOT, 1000), (BOB, LDOT, 1000)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
