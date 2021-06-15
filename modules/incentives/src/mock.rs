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

//! Mocks for the incentives module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime,
	dispatch::{DispatchError, DispatchResult},
	ord_parameter_types, parameter_types,
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{DexShare, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{testing::Header, traits::IdentityLookup};
use sp_std::cell::RefCell;
pub use support::{CDPTreasury, DEXManager, Price, Ratio};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const VAULT: AccountId = 10;
pub const UNRELEASED: AccountId = 11;
pub const VALIDATOR: AccountId = 3;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const BTC: CurrencyId = CurrencyId::Token(TokenSymbol::RENBTC);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const BTC_AUSD_LP: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::RENBTC), DexShare::Token(TokenSymbol::AUSD));
pub const DOT_AUSD_LP: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::DOT), DexShare::Token(TokenSymbol::AUSD));

mod incentives {
	pub use super::super::*;
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

pub struct MockCDPTreasury;
impl CDPTreasury<AccountId> for MockCDPTreasury {
	type Balance = Balance;
	type CurrencyId = CurrencyId;

	fn get_surplus_pool() -> Balance {
		unimplemented!()
	}

	fn get_debit_pool() -> Balance {
		unimplemented!()
	}

	fn get_total_collaterals(_: CurrencyId) -> Balance {
		unimplemented!()
	}

	fn get_debit_proportion(_: Balance) -> Ratio {
		unimplemented!()
	}

	fn on_system_debit(_: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn on_system_surplus(_: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn issue_debit(who: &AccountId, debit: Balance, _: bool) -> DispatchResult {
		TokensModule::deposit(AUSD, who, debit)
	}

	fn burn_debit(_: &AccountId, _: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn deposit_surplus(_: &AccountId, _: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn deposit_collateral(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn withdraw_collateral(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		unimplemented!()
	}
}

pub struct MockDEX;
impl DEXManager<AccountId, CurrencyId, Balance> for MockDEX {
	fn get_liquidity_pool(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance) {
		match (currency_id_a, currency_id_b) {
			(AUSD, BTC) => (500, 100),
			(AUSD, DOT) => (400, 100),
			(BTC, AUSD) => (100, 500),
			(DOT, AUSD) => (100, 400),
			_ => (0, 0),
		}
	}

	fn get_liquidity_token_address(_currency_id_a: CurrencyId, _currency_id_b: CurrencyId) -> Option<H160> {
		unimplemented!()
	}

	fn get_swap_target_amount(_: &[CurrencyId], _: Balance, _: Option<Ratio>) -> Option<Balance> {
		unimplemented!()
	}

	fn get_swap_supply_amount(_: &[CurrencyId], _: Balance, _: Option<Ratio>) -> Option<Balance> {
		unimplemented!()
	}

	fn swap_with_exact_supply(
		_: &AccountId,
		_: &[CurrencyId],
		_: Balance,
		_: Balance,
		_: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		unimplemented!()
	}

	fn swap_with_exact_target(
		_: &AccountId,
		_: &[CurrencyId],
		_: Balance,
		_: Balance,
		_: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		unimplemented!()
	}

	fn add_liquidity(
		_: &AccountId,
		_: CurrencyId,
		_: CurrencyId,
		_: Balance,
		_: Balance,
		_: Balance,
		_: bool,
	) -> DispatchResult {
		unimplemented!()
	}

	fn remove_liquidity(
		_: &AccountId,
		_: CurrencyId,
		_: CurrencyId,
		_: Balance,
		_: Balance,
		_: Balance,
		_: bool,
	) -> DispatchResult {
		unimplemented!()
	}
}

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

pub fn mock_shutdown() {
	IS_SHUTDOWN.with(|v| *v.borrow_mut() = true)
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		IS_SHUTDOWN.with(|v| *v.borrow_mut())
	}
}

impl orml_rewards::Config for Runtime {
	type Share = Balance;
	type Balance = Balance;
	type PoolId = PoolId<AccountId>;
	type Handler = IncentivesModule;
}

parameter_types! {
	pub const RewardsVaultAccountId: AccountId = VAULT;
	pub const NativeRewardsSource: AccountId = UNRELEASED;
	pub const AccumulatePeriod: BlockNumber = 10;
	pub const NativeCurrencyId: CurrencyId = ACA;
	pub const StableCurrencyId: CurrencyId = AUSD;
	pub const LiquidCurrencyId: CurrencyId = LDOT;
	pub const IncentivesPalletId: PalletId = PalletId(*b"aca/inct");
}

ord_parameter_types! {
	pub const Four: AccountId = 4;
}

impl Config for Runtime {
	type Event = Event;
	type RelaychainAccountId = AccountId;
	type RewardsVaultAccountId = RewardsVaultAccountId;
	type NativeRewardsSource = NativeRewardsSource;
	type AccumulatePeriod = AccumulatePeriod;
	type NativeCurrencyId = NativeCurrencyId;
	type StableCurrencyId = StableCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type UpdateOrigin = EnsureSignedBy<Four, AccountId>;
	type CDPTreasury = MockCDPTreasury;
	type Currency = TokensModule;
	type DEX = MockDEX;
	type EmergencyShutdown = MockEmergencyShutdown;
	type PalletId = IncentivesPalletId;
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
		IncentivesModule: incentives::{Pallet, Storage, Call, Event<T>},
		TokensModule: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		RewardsModule: orml_rewards::{Pallet, Storage, Call},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![(UNRELEASED, ACA, 10_000)],
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
