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

//! Mocks for the incentives module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime,
	dispatch::{DispatchError, DispatchResult},
	ord_parameter_types, parameter_types,
	traits::{ConstU64, Everything, Nothing},
	weights::constants::RocksDbWeight,
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{DexShare, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32};
use sp_std::cell::RefCell;
pub use support::{CDPTreasury, DEXManager, Price, Ratio, SwapLimit};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

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

ord_parameter_types! {
	pub const ALICE: AccountId = AccountId::from([1u8; 32]);
	pub const BOB: AccountId = AccountId::from([2u8; 32]);
	pub const VAULT: AccountId = IncentivesModule::account_id();
	pub const RewardsSource: AccountId = AccountId::from([3u8; 32]);
	pub const ROOT: AccountId = AccountId32::new([255u8; 32]);
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
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = RocksDbWeight;
	type BaseCallFilter = Everything;
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
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
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
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

	fn withdraw_surplus(_: &AccountId, _: Balance) -> DispatchResult {
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
impl DEXManager<AccountId, Balance, CurrencyId> for MockDEX {
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

	fn get_swap_amount(_: &[CurrencyId], _: SwapLimit<Balance>) -> Option<(Balance, Balance)> {
		unimplemented!()
	}

	fn get_best_price_swap_path(
		_: CurrencyId,
		_: CurrencyId,
		_: SwapLimit<Balance>,
		_: Vec<Vec<CurrencyId>>,
	) -> Option<(Vec<CurrencyId>, Balance, Balance)> {
		unimplemented!()
	}

	fn swap_with_specific_path(
		_: &AccountId,
		_: &[CurrencyId],
		_: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
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
	) -> sp_std::result::Result<(Balance, Balance, Balance), DispatchError> {
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
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
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
	type PoolId = PoolId;
	type CurrencyId = CurrencyId;
	type Handler = IncentivesModule;
}

parameter_types! {
	pub const StableCurrencyId: CurrencyId = AUSD;
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const IncentivesPalletId: PalletId = PalletId(*b"aca/inct");
}

ord_parameter_types! {
	pub const Root: AccountId = ROOT::get();
	pub const EarnShareBooster: Permill = Permill::from_percent(50);
}

impl Config for Runtime {
	type Event = Event;
	type RewardsSource = RewardsSource;
	type AccumulatePeriod = ConstU64<10>;
	type StableCurrencyId = StableCurrencyId;
	type NativeCurrencyId = GetNativeCurrencyId;
	type EarnShareBooster = EarnShareBooster;
	type UpdateOrigin = EnsureSignedBy<ROOT, AccountId>;
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
		Self { balances: vec![] }
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
