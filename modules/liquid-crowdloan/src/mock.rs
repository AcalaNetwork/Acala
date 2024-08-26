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

//! Mocks for example module.

#![cfg(test)]

use super::*;
use crate as liquid_crowdloan;

use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use module_support::mocks::MockAddressMapping;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, TokenSymbol};
use sp_core::H160;
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const LCDOT: CurrencyId = CurrencyId::LiquidCrowdloan(13);

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);

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
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
	pub LiquidCrowdloanPalletId: PalletId = PalletId(*b"aca/lqcl");
	pub const GetLDOT: CurrencyId = LDOT;
	pub const GetDOT: CurrencyId = DOT;
	pub const GetLCDOT: CurrencyId = LCDOT;
}

impl module_currencies::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
	type GasToWeight = ();
	type SweepOrigin = EnsureRoot<AccountId>;
	type OnDust = ();
}

parameter_types! {
	pub static TransferRecord: Option<(AccountId, AccountId, Balance)> = None;
	pub static TransferOk: bool = true;
}

ord_parameter_types! {
	pub const Alice: AccountId = ALICE;
}

impl liquid_crowdloan::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type LiquidCrowdloanCurrencyId = GetLCDOT;
	type RelayChainCurrencyId = GetDOT;
	type PalletId = LiquidCrowdloanPalletId;
	type GovernanceOrigin = EnsureSignedBy<Alice, AccountId>;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: module_currencies,
		LiquidCrowdloan: liquid_crowdloan,
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
	transfer_ok: bool,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![],
			transfer_ok: true,
		}
	}
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn transfer_ok(mut self, transfer_ok: bool) -> Self {
		self.transfer_ok = transfer_ok;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		TransferRecord::mutate(|v| *v = None);
		TransferOk::mutate(|v| *v = self.transfer_ok);

		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == ACA)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != ACA)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
