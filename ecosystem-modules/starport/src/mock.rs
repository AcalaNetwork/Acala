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

//! Mocks for the Starport module.

#![cfg(test)]

use super::*;
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{ConstU32, ConstU64, Everything, Nothing},
};
use frame_system::EnsureSignedBy;
use module_support::mocks::MockAddressMapping;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
use crate as ecosystem_starport;

mod starport {
	pub use super::super::*;
}

pub const GATEWAY_ACCOUNT: AccountId = AccountId32::new([11u8; 32]);
pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const ACALA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const KSM: CurrencyId = CurrencyId::Token(TokenSymbol::KSM);
pub const CASH: CurrencyId = CurrencyId::Token(TokenSymbol::CASH);
pub const INITIAL_BALANCE: Balance = 1000000;

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
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
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
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
	type GasToWeight = ();
	type SweepOrigin = EnsureSignedBy<One, AccountId>;
	type OnDust = ();
}

pub struct MockCashModule;
impl CompoundCashTrait<Balance, Moment> for MockCashModule {
	fn set_future_yield(
		_next_cash_yield: Balance,
		_yield_index: CashYieldIndex,
		_timestamp_effective: Moment,
	) -> DispatchResult {
		Ok(())
	}
}

pub const MAX_GATEWAY_AUTHORITIES: u32 = 5;
pub const PERCENT_THRESHOLD_FOR_AUTHORITY_SIGNATURE: Perbill = Perbill::from_percent(50);

parameter_types! {
	pub const GatewayAccount: AccountId = GATEWAY_ACCOUNT;
	pub const CashCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::CASH);
	pub const StarportPalletId: PalletId = PalletId(*b"aca/stpt");
	pub const MaxGatewayAuthorities: u32 = MAX_GATEWAY_AUTHORITIES;
	pub const PercentThresholdForAuthoritySignature: Perbill = PERCENT_THRESHOLD_FOR_AUTHORITY_SIGNATURE;
}

impl Config for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type CashCurrencyId = CashCurrencyId;
	type PalletId = StarportPalletId;
	type MaxGatewayAuthorities = MaxGatewayAuthorities;
	type PercentThresholdForAuthoritySignature = PercentThresholdForAuthoritySignature;
	type Cash = MockCashModule;
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
		Starport: ecosystem_starport::{Pallet, Call, Storage, Event<T>},
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
		Self {
			tokens_balances: vec![(ALICE, KSM, INITIAL_BALANCE), (ALICE, CASH, INITIAL_BALANCE)],
			native_balances: vec![(ALICE, INITIAL_BALANCE)],
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

		GenesisBuild::<Runtime>::assimilate_storage(
			&ecosystem_starport::GenesisConfig {
				initial_authorities: get_mock_signatures(),
			},
			&mut t,
		)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

/// Returns a Vec of mock signatures
pub fn get_mock_signatures() -> Vec<CompoundAuthoritySignature> {
	vec![
		AccountId::new([0xF1; 32]),
		AccountId::new([0xF2; 32]),
		AccountId::new([0xF3; 32]),
	]
}
