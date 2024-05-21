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

//! Mocks for homa validator list module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, derive_impl, parameter_types,
	traits::{ConstU128, ConstU32, Nothing},
};
use frame_system::EnsureRoot;
use module_support::ExchangeRate;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, Balance, CurrencyId, TokenSymbol};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const CHARLIE: AccountId = 2;
pub const VALIDATOR_1: AccountId = 11;
pub const VALIDATOR_2: AccountId = 12;
pub const VALIDATOR_3: AccountId = 13;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

mod homa_validator_list {
	pub use super::super::*;
}

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
	type MaxFreezes = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
}

pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;
pub type LDOTCurrency = orml_currencies::Currency<Runtime, GetLiquidCurrencyId>;

impl orml_currencies::Config for Runtime {
	type MultiCurrency = OrmlTokens;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

pub struct MockLiquidStakingExchangeProvider;
impl ExchangeRateProvider for MockLiquidStakingExchangeProvider {
	fn get_exchange_rate() -> ExchangeRate {
		ExchangeRate::saturating_from_rational(1, 2)
	}
}

parameter_types! {
	pub static MockCurrentEra: EraIndex = 0;
	pub ActiveSubAccountsIndexList: Vec<u16> = vec![0, 1, 2];
	pub const BondingDuration: EraIndex = 28;
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RelayChainAccountId = AccountId;
	type LiquidTokenCurrency = LDOTCurrency;
	type MinBondAmount = ConstU128<100>;
	type BondingDuration = BondingDuration;
	type ValidatorInsuranceThreshold = ConstU128<200>;
	type GovernanceOrigin = EnsureRoot<AccountId>;
	type LiquidStakingExchangeRateProvider = MockLiquidStakingExchangeProvider;
	type CurrentEra = MockCurrentEra;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		OrmlTokens: orml_tokens,
		PalletBalances: pallet_balances,
		OrmlCurrencies: orml_currencies,
		HomaValidatorListModule: homa_validator_list,
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![(ALICE, LDOT, 1000), (BOB, LDOT, 1000), (CHARLIE, LDOT, 1000000)],
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

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext
	}
}
