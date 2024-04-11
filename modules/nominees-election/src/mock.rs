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

//! Mocks for nominees election module.

#![cfg(test)]

use super::*;

use crate as nominees_election;
use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, Nothing},
};
use orml_traits::parameter_type_with_key;
use primitives::{Amount, CurrencyId, TokenSymbol};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

ord_parameter_types! {
	pub const One: AccountId = ALICE;
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ConstU32<100>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxHolds = ();
	type MaxFreezes = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
}

pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;
pub type LDOTCurrency = orml_currencies::Currency<Runtime, GetLiquidCurrencyId>;

impl orml_currencies::Config for Runtime {
	type MultiCurrency = TokensModule;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

parameter_types! {
	pub const PalletId: LockIdentifier = *b"1       ";
}

pub struct MockNomineeFilter;
impl Contains<AccountId> for MockNomineeFilter {
	fn contains(a: &AccountId) -> bool {
		(0..=6).contains(a)
	}
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = LDOTCurrency;
	type NomineeId = AccountId;
	type PalletId = PalletId;
	type MinBond = ConstU128<5>;
	type BondingDuration = ConstU32<4>;
	type NominateesCount = ConstU32<5>;
	type MaxUnbondingChunks = ConstU32<3>;
	type NomineeFilter = MockNomineeFilter;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		NomineesElectionModule: nominees_election,
		TokensModule: orml_tokens,
		PalletBalances: pallet_balances,
		OrmlCurrencies: orml_currencies,
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
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
