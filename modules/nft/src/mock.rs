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

#![cfg(test)]

use super::*;

use crate as nft;
use codec::{Decode, Encode};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{Filter, InstanceFilter},
	RuntimeDebug,
};
use orml_traits::parameter_type_with_key;
use primitives::{Amount, Balance, BlockNumber, CurrencyId, ReserveIdentifier, TokenSymbol};
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use support::mocks::MockAddressMapping;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

pub type AccountId = AccountId32;

impl frame_system::Config for Runtime {
	type BaseCallFilter = BaseFilter;
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Call = Call;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const MaxReserves: u32 = 50;
}
impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
}
impl pallet_utility::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type WeightInfo = ();
}
parameter_types! {
	pub const ProxyDepositBase: u64 = 1;
	pub const ProxyDepositFactor: u64 = 1;
	pub const MaxProxies: u16 = 4;
	pub const MaxPending: u32 = 2;
	pub const AnnouncementDepositBase: u64 = 1;
	pub const AnnouncementDepositFactor: u64 = 1;
}
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen)]
pub enum ProxyType {
	Any,
	JustTransfer,
	JustUtility,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}
impl InstanceFilter<Call> for ProxyType {
	fn filter(&self, c: &Call) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::JustTransfer => matches!(c, Call::Balances(pallet_balances::Call::transfer(..))),
			ProxyType::JustUtility => matches!(c, Call::Utility(..)),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		self == &ProxyType::Any || self == o
	}
}
pub struct BaseFilter;
impl Filter<Call> for BaseFilter {
	fn filter(c: &Call) -> bool {
		match *c {
			// Remark is used as a no-op call in the benchmarking
			Call::System(SystemCall::remark(_)) => true,
			Call::System(_) => false,
			_ => true,
		}
	}
}
impl pallet_proxy::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = ();
	type CallHasher = BlakeTwo256;
	type MaxPending = MaxPending;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

pub type NativeCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

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

pub const NATIVE_CURRENCY_ID: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
}

parameter_types! {
	pub const CreateClassDeposit: Balance = 200;
	pub const CreateTokenDeposit: Balance = 100;
	pub const DataDepositPerByte: Balance = 10;
	pub const NftPalletId: PalletId = PalletId(*b"aca/aNFT");
}
impl Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type CreateClassDeposit = CreateClassDeposit;
	type CreateTokenDeposit = CreateTokenDeposit;
	type DataDepositPerByte = DataDepositPerByte;
	type PalletId = NftPalletId;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxClassMetadata: u32 = 1024;
	pub const MaxTokenMetadata: u32 = 1024;
}

impl orml_nft::Config for Runtime {
	type ClassId = u32;
	type TokenId = u64;
	type ClassData = ClassData<Balance>;
	type TokenData = TokenData<Balance>;
	type MaxClassMetadata = MaxClassMetadata;
	type MaxTokenMetadata = MaxTokenMetadata;
}

use frame_system::Call as SystemCall;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		NFTModule: nft::{Pallet, Call, Event<T>},
		OrmlNFT: orml_nft::{Pallet, Storage, Config<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>},
		Utility: pallet_utility::{Pallet, Call, Event},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currency: module_currencies::{Pallet, Call, Event<T>},
	}
);

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const CLASS_ID: <Runtime as orml_nft::Config>::ClassId = 0;
pub const CLASS_ID_NOT_EXIST: <Runtime as orml_nft::Config>::ClassId = 1;
pub const TOKEN_ID: <Runtime as orml_nft::Config>::TokenId = 0;
pub const TOKEN_ID_NOT_EXIST: <Runtime as orml_nft::Config>::TokenId = 1;

pub struct ExtBuilder;
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
			balances: vec![(ALICE, 100000)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
