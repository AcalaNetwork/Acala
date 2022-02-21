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

//! Mocks for the Account Tokenizer module

#![cfg(test)]

use super::*;
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{Everything, InstanceFilter},
	PalletId,
};
use frame_system::EnsureSignedBy;
use module_support::ProxyXcm;
use primitives::ReserveIdentifier;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	AccountId32,
};

use module_foreign_state_oracle::EnsureForeignStateOracle;
use module_nft::{ClassData, TokenData};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

mod account_tokenizer {
	pub use super::super::*;
}

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const TREASURY: AccountId = AccountId32::new([255u8; 32]);

pub fn dollar(b: Balance) -> Balance {
	b * 1_000_000_000_000
}

/// mock XCM transfer.
pub struct MockProxyXcm;
impl ProxyXcm<AccountId> for MockProxyXcm {
	fn transfer_proxy(_real: AccountId, _new_owner: AccountId) -> DispatchResult {
		Ok(())
	}

	fn get_transfer_proxy_xcm_fee() -> Balance {
		0
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const NativeTokenExistentialDeposit: Balance = 0;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = NativeTokenExistentialDeposit;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ReserveIdentifier;
}

parameter_types! {
	pub CreateClassDeposit: Balance = 0;
	pub CreateTokenDeposit: Balance = 0;
	pub MaxAttributesBytes: u32 = 2048;
	pub const NftPalletId: PalletId = PalletId(*b"aca/mnft");
	pub const DataDepositPerByte: Balance = 10;
}

impl module_nft::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type CreateClassDeposit = CreateClassDeposit;
	type CreateTokenDeposit = CreateTokenDeposit;
	type DataDepositPerByte = DataDepositPerByte;
	type PalletId = NftPalletId;
	type MaxAttributesBytes = MaxAttributesBytes;
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum MockProxyType {
	Any,
}
impl Default for MockProxyType {
	fn default() -> Self {
		Self::Any
	}
}
impl InstanceFilter<Call> for MockProxyType {
	fn filter(&self, _c: &Call) -> bool {
		true
	}
	fn is_superset(&self, _o: &Self) -> bool {
		true
	}
}

parameter_types! {
	pub const ProxyDepositBase: u64 = 0;
	pub const ProxyDepositFactor: u64 = 0;
	pub const MaxProxies: u16 = 4;
	pub const MaxPending: u32 = 2;
	pub const AnnouncementDepositBase: u64 = 0;
	pub const AnnouncementDepositFactor: u64 = 0;
}

impl pallet_proxy::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type ProxyType = MockProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = ();
	type CallHasher = BlakeTwo256;
	type MaxPending = MaxPending;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

parameter_types! {
	pub const ForeignOraclePalletId: PalletId = PalletId(*b"aca/fsto");
	pub const QueryDuration: BlockNumber = 10;
	pub const QueryFee: Balance = 100;
	pub const CancelFee: Balance = 10;
}

ord_parameter_types! {
	pub const One: AccountId = AccountId32::new([1; 32]);
}

impl module_foreign_state_oracle::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type VerifiableTask = Call;
	type QueryFee = QueryFee;
	type CancelFee = CancelFee;
	type OracleOrigin = EnsureSignedBy<One, AccountId>;
	type QueryDuration = QueryDuration;
	type Currency = Balances;
	type PalletId = ForeignOraclePalletId;
	type BlockNumberProvider = System;
}

parameter_types! {
	pub const AcccountTokenizerPalletId: PalletId = PalletId(*b"aca/atnz");
	pub AccountTokenizerPalletAccount: AccountId = AcccountTokenizerPalletId::get().into_account();
	pub TreasuryAccount: AccountId = TREASURY;
	pub MintRequestDeposit: Balance = dollar(1);
	pub MintFee: Balance = dollar(1);
}

impl Config for Runtime {
	type Event = Event;
	type PalletAccount = AccountTokenizerPalletAccount;
	type Currency = Balances;
	type XcmInterface = MockProxyXcm;
	type OracleOrigin = EnsureForeignStateOracle;
	type NFTInterface = ModuleNFT;
	type TreasuryAccount = TreasuryAccount;
	type MintRequestDeposit = MintRequestDeposit;
	type MintFee = MintFee;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},

		ModuleNFT: module_nft::{Pallet, Call, Event<T>},
		OrmlNFT: orml_nft::{Pallet, Storage, Config<T>},
		Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>},
		ForeignStateOracle: module_foreign_state_oracle::{Pallet, Call, Storage, Event<T>, Origin},
		AccountTokenizer: account_tokenizer::{Pallet, Call, Storage, Event<T>},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { balances: vec![] }
	}
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.map(|(account_id, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
