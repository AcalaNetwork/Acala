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

//! Mocks for the Starport module.

#![cfg(test)]

use super::*;
use frame_support::{ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use module_support::mocks::MockAddressMapping;
use orml_traits::{parameter_type_with_key, XcmExecutionResult, XcmTransfer};
use primitives::{Amount, TokenSymbol};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32};
use xcm::opaque::v0::{Junction, MultiAsset, MultiLocation, NetworkId, Outcome};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
use crate as module_homa_lite;

mod homa_lite {
	pub use super::super::*;
}

pub const ROOT: AccountId = AccountId32::new([255u8; 32]);
pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const INVALID_CALLER: AccountId = AccountId32::new([254u8; 32]);
pub const ACALA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const KSM: CurrencyId = CurrencyId::Token(TokenSymbol::KSM);
pub const LKSM: CurrencyId = CurrencyId::Token(TokenSymbol::LKSM);
pub const INITIAL_BALANCE: Balance = 1_000_000;
pub const MOCK_XCM_DESTINATION: MultiLocation = MultiLocation::X1(Junction::AccountId32 {
	network: NetworkId::Kusama,
	id: [1u8; 32],
});

/// For testing only. Does not check for overflow.
pub fn dollar(b: Balance) -> Balance {
	b * 1_000_000_000_000
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

/// A mock XCM transfer.
/// Only fails if it is called by "INVALID_CALLER". Otherwise returns OK with 0 weight.
pub struct MockXcm;
impl XcmTransfer<AccountId, Balance, CurrencyId> for MockXcm {
	fn transfer(
		who: AccountId,
		_currency_id: CurrencyId,
		_amount: Balance,
		_dest: MultiLocation,
		_dest_weight: Weight,
	) -> XcmExecutionResult {
		match who {
			INVALID_CALLER => Ok(Outcome::Error(xcm::opaque::v0::Error::Undefined)),
			_ => Ok(Outcome::Complete(0)),
		}
	}

	/// Transfer `MultiAsset`
	fn transfer_multi_asset(
		_who: AccountId,
		_asset: MultiAsset,
		_dest: MultiLocation,
		_dest_weight: Weight,
	) -> XcmExecutionResult {
		Ok(Outcome::Complete(0))
	}
}

impl frame_system::Config for Runtime {
	type BaseCallFilter = ();
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
	type ReserveIdentifier = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACALA;
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
}

parameter_types! {
	pub const StakingCurrencyId: CurrencyId = KSM;
	pub const LiquidCurrencyId: CurrencyId = LKSM;
	pub const MinimumMintThreshold: Balance = 1_000_000_000;
	pub const MockXcmDestination: MultiLocation = MOCK_XCM_DESTINATION;
}
ord_parameter_types! {
	pub const Root: AccountId = ROOT;
}

impl Config for Runtime {
	type Event = Event;
	type WeightInfo = ();
	type Currency = Currencies;
	type StakingCurrencyId = StakingCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type IssuerOrigin = EnsureSignedBy<Root, AccountId>;
	type GovernanceOrigin = EnsureSignedBy<Root, AccountId>;
	type MinimumMintThreshold = MinimumMintThreshold;
	type XcmTransfer = MockXcm;
	type SovereignSubAccountLocation = MockXcmDestination;
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
		HomaLite: module_homa_lite::{Pallet, Call, Storage, Event<T>},
		PalletBalances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
	}
);

pub struct ExtBuilder {
	tokens_balances: Vec<(AccountId, CurrencyId, Balance)>,
	native_balances: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		let initial = dollar(INITIAL_BALANCE);
		Self {
			tokens_balances: vec![
				(ALICE, KSM, initial),
				(BOB, KSM, initial),
				(ROOT, LKSM, initial),
				(INVALID_CALLER, KSM, initial),
			],
			native_balances: vec![
				(ALICE, initial),
				(BOB, initial),
				(ROOT, initial),
				(INVALID_CALLER, initial),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self.native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.tokens_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
