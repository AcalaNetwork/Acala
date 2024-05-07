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

//! Mocks for the incentives module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU64, Nothing},
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{DexShare, TokenSymbol};
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};

pub type AccountId = AccountId32;

pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const BTC: CurrencyId = CurrencyId::ForeignAsset(255);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const BTC_AUSD_LP: CurrencyId =
	CurrencyId::DexShare(DexShare::ForeignAsset(255), DexShare::Token(TokenSymbol::AUSD));
pub const DOT_AUSD_LP: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::DOT), DexShare::Token(TokenSymbol::AUSD));

mod incentives {
	pub use super::super::*;
}

ord_parameter_types! {
	pub const ALICE: AccountId = AccountId::from([1u8; 32]);
	pub const BOB: AccountId = AccountId::from([2u8; 32]);
	pub const VAULT: AccountId = IncentivesModule::account_id();
	pub const RewardsSource: AccountId = AccountId::from([3u8; 32]);
	pub const ROOT: AccountId = AccountId32::new([255u8; 32]);
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {

		match currency_id {
			CurrencyId::Token(TokenSymbol::AUSD) => 10,
			_ => Default::default()
		}
	};
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
}

parameter_types! {
	static IsShutdown: bool = false;
}

pub fn mock_shutdown() {
	IsShutdown::mutate(|v| *v = true)
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		IsShutdown::get()
	}
}

parameter_type_with_key! {
	pub MinimalShares: |_pool_id: PoolId| -> Balance {
		0
	};
}

impl orml_rewards::Config for Runtime {
	type Share = Balance;
	type Balance = Balance;
	type PoolId = PoolId;
	type CurrencyId = CurrencyId;
	type MinimalShares = MinimalShares;
	type Handler = IncentivesModule;
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const IncentivesPalletId: PalletId = PalletId(*b"aca/inct");
}

ord_parameter_types! {
	pub const Root: AccountId = ROOT::get();
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RewardsSource = RewardsSource;
	type AccumulatePeriod = ConstU64<10>;
	type NativeCurrencyId = GetNativeCurrencyId;
	type UpdateOrigin = EnsureSignedBy<ROOT, AccountId>;
	type Currency = TokensModule;
	type EmergencyShutdown = MockEmergencyShutdown;
	type PalletId = IncentivesPalletId;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		IncentivesModule: incentives,
		TokensModule: orml_tokens,
		RewardsModule: orml_rewards,
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { balances: vec![] }
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

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
