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

//! Mocks for the chainsafe module.

#![cfg(test)]

use super::*;
use frame_support::{construct_runtime, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, TokenSymbol};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};

pub type BlockNumber = u64;
pub type AccountId = u128;

pub const ALICE: AccountId = 1;

mod ecosystem_chainsafe {
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
	type AccountData = ();
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

impl orml_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
}

ord_parameter_types! {
	pub const AdminOrigin: AccountId = 11;
}

parameter_types! {
	pub const LocalChainId: chainbridge::ChainId = 2;
	pub const ProposalLifetime: BlockNumber = 10;
	pub ACAResourceId: chainbridge::ResourceId = chainbridge::derive_resource_id(LocalChainId::get(), b"aca");
	pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
	pub WETHResourceId: chainbridge::ResourceId = chainbridge::derive_resource_id(0, b"weth");
	pub WETH: CurrencyId = CurrencyId::ChainSafe(WETHResourceId::get());
}

impl chainbridge::Config for Runtime {
	type Event = Event;
	type AdminOrigin = EnsureSignedBy<AdminOrigin, AccountId>;
	type Proposal = Call;
	type ChainId = LocalChainId;
	type ProposalLifetime = ProposalLifetime;
}

ord_parameter_types! {
	pub const RegistorOrigin: AccountId = 12;
}

impl Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type NativeCurrencyId = ACA;
	type RegistorOrigin = EnsureSignedBy<RegistorOrigin, AccountId>;
	type BridgeOrigin = chainbridge::EnsureBridge<Runtime>;
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
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>},
		ChainSafeTransfer: ecosystem_chainsafe::{Pallet, Call, Storage, Event<T>},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![(ALICE, ACA::get(), 1_000u128)],
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
