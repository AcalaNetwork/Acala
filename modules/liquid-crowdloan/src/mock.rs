// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use orml_traits::parameter_type_with_key;
use primitives::{Amount, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32};
use std::cell::RefCell;
use support::mocks::MockAddressMapping;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const VAULT: AccountId = AccountId32::new([3u8; 32]);

impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
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
	type ExistentialDeposit = ConstU128<0>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
	pub CrowdloanVault: AccountId = VAULT;
	pub LiquidCrowdloanPalletId: PalletId = PalletId(*b"aca/lqcl");
	pub const Ldot: CurrencyId = LDOT;
	pub const Dot: CurrencyId = DOT;
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

thread_local! {
	pub static TRANSFER_RECORD: RefCell<Option<(AccountId, AccountId, Balance)>> = RefCell::new(None);
	pub static TRANSFER_OK: RefCell<bool> = RefCell::new(true);
}

pub struct MockXcmTransfer;
impl CrowdloanVaultXcm<AccountId, Balance> for MockXcmTransfer {
	fn transfer_to_liquid_crowdloan_module_account(
		vault: AccountId,
		recipient: AccountId,
		amount: Balance,
	) -> DispatchResult {
		if TRANSFER_OK.with(|v| *v.borrow()) {
			TRANSFER_RECORD.with(|v| *v.borrow_mut() = Some((vault, recipient, amount)));
			Ok(())
		} else {
			Err(DispatchError::Other("transfer failed"))
		}
	}
}

ord_parameter_types! {
	pub const Alice: AccountId = ALICE;
}

impl liquid_crowdloan::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type LiquidCrowdloanCurrencyId = Ldot;
	type RelayChainCurrencyId = Dot;
	type PalletId = LiquidCrowdloanPalletId;
	type GovernanceOrigin = EnsureSignedBy<Alice, AccountId>;
	type CrowdloanVault = CrowdloanVault;
	type XcmTransfer = MockXcmTransfer;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
		LiquidCrowdloan: liquid_crowdloan::{Pallet, Call, Event<T>, Storage},
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
		TRANSFER_RECORD.with(|v| *v.borrow_mut() = None);
		TRANSFER_OK.with(|v| *v.borrow_mut() = self.transfer_ok);

		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
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
