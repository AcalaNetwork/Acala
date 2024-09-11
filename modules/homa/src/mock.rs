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

//! Mocks for the Homa module.

#![cfg(test)]

use super::*;
use frame_support::{
	derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use module_support::mocks::MockAddressMapping;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, TokenSymbol};
use sp_core::H160;
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};
use xcm::v4::prelude::*;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

mod homa {
	pub use super::super::*;
}

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId32::new([3u8; 32]);
pub const DAVE: AccountId = AccountId32::new([4u8; 32]);
pub const HOMA_TREASURY: AccountId = AccountId32::new([255u8; 32]);
pub const NATIVE_CURRENCY_ID: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const STAKING_CURRENCY_ID: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LIQUID_CURRENCY_ID: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const VALIDATOR_A: AccountId = AccountId32::new([200u8; 32]);
pub const VALIDATOR_B: AccountId = AccountId32::new([201u8; 32]);
pub const VALIDATOR_C: AccountId = AccountId32::new([202u8; 32]);
pub const VALIDATOR_D: AccountId = AccountId32::new([203u8; 32]);

/// mock XCM transfer.
pub struct MockHomaSubAccountXcm;
impl HomaSubAccountXcm<AccountId, Balance> for MockHomaSubAccountXcm {
	type RelayChainAccountId = AccountId;

	fn transfer_staking_to_sub_account(sender: &AccountId, _: u16, amount: Balance) -> DispatchResult {
		Currencies::withdraw(StakingCurrencyId::get(), sender, amount)
	}

	fn withdraw_unbonded_from_sub_account(_: u16, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn bond_extra_on_sub_account(_: u16, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn unbond_on_sub_account(_: u16, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn nominate_on_sub_account(_: u16, _: Vec<Self::RelayChainAccountId>) -> DispatchResult {
		Ok(())
	}

	fn get_xcm_transfer_fee() -> Balance {
		1_000_000
	}

	fn get_parachain_fee(_: Location) -> Balance {
		1_000_000
	}
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
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
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

impl BlockNumberProvider for MockRelayBlockNumberProvider {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		Self::get()
	}
}

ord_parameter_types! {
	pub const HomaAdmin: AccountId = DAVE;
}

parameter_types! {
	pub const StakingCurrencyId: CurrencyId = STAKING_CURRENCY_ID;
	pub const LiquidCurrencyId: CurrencyId = LIQUID_CURRENCY_ID;
	pub const HomaPalletId: PalletId = PalletId(*b"aca/homa");
	pub const TreasuryAccount: AccountId = HOMA_TREASURY;
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub ActiveSubAccountsIndexList: Vec<u16> = vec![0, 1, 2];
	pub const BondingDuration: EraIndex = 28;
	pub static MintThreshold: Balance = 0;
	pub static RedeemThreshold: Balance = 0;
	pub static MockRelayBlockNumberProvider: BlockNumber = 0;
}

pub struct MockNominationsProvider;
impl NomineesProvider<AccountId> for MockNominationsProvider {
	fn nominees() -> Vec<AccountId> {
		unimplemented!()
	}

	fn nominees_in_groups(group_index_list: Vec<u16>) -> Vec<(u16, Vec<AccountId>)> {
		group_index_list
			.iter()
			.map(|group_index| {
				let nominees: Vec<AccountId> = match *group_index {
					0 => vec![VALIDATOR_A, VALIDATOR_B],
					2 => vec![VALIDATOR_A, VALIDATOR_C],
					3 => vec![VALIDATOR_D],
					_ => vec![],
				};
				(*group_index, nominees)
			})
			.collect()
	}
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GovernanceOrigin = EnsureSignedBy<HomaAdmin, AccountId>;
	type StakingCurrencyId = StakingCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type PalletId = HomaPalletId;
	type TreasuryAccount = TreasuryAccount;
	type DefaultExchangeRate = DefaultExchangeRate;
	type ActiveSubAccountsIndexList = ActiveSubAccountsIndexList;
	type BondingDuration = BondingDuration;
	type MintThreshold = MintThreshold;
	type RedeemThreshold = RedeemThreshold;
	type RelayChainBlockNumber = MockRelayBlockNumberProvider;
	type XcmInterface = MockHomaSubAccountXcm;
	type WeightInfo = ();
	type NominationsProvider = MockNominationsProvider;
	type ProcessRedeemRequestsLimit = ConstU32<3>;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Homa: homa,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: module_currencies,
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
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == NATIVE_CURRENCY_ID)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != NATIVE_CURRENCY_ID)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
