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

//! Mocks for the Homa module.

#![cfg(test)]

use super::*;
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use module_support::mocks::MockAddressMapping;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32};
use xcm::latest::prelude::*;

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

/// mock XCM transfer.
pub struct MockHomaSubAccountXcm;
impl HomaSubAccountXcm<AccountId, Balance> for MockHomaSubAccountXcm {
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

	fn get_xcm_transfer_fee() -> Balance {
		1_000_000
	}

	fn get_parachain_fee(_: MultiLocation) -> Balance {
		1_000_000
	}
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

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<0>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
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

impl Config for Runtime {
	type Event = Event;
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
		Homa: homa::{Pallet, Call, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
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
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
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
