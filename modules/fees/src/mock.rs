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

//! Mocks for asset fee distribution module.

#![cfg(test)]

use crate as fees;
use frame_support::{
	construct_runtime, ord_parameter_types,
	pallet_prelude::*,
	parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
	PalletId,
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{
	AccountId, Amount, Balance, BlockNumber, CurrencyId, IncomeSource, ReserveIdentifier, TokenSymbol, TradingPair,
};
use sp_core::H160;
use sp_runtime::traits::AccountIdConversion;
use support::mocks::MockAddressMapping;

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId::new([3u8; 32]);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);

impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = sp_runtime::testing::H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type Header = sp_runtime::testing::Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
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

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
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
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type WeightInfo = ();
	type MaxLocks = ConstU32<100>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
}

ord_parameter_types! {
	pub const ListingOrigin: AccountId = ALICE;
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type EVMBridge = ();
	type GasToWeight = ();
	type SweepOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type OnDust = ();
}

parameter_types! {
	pub TreasuryAccount: AccountId = PalletId(*b"aca/trsy").into_account_truncating();
	// Treasury pools
	pub NetworkTreasuryPool: AccountId = PalletId(*b"aca/nktp").into_account_truncating();
	pub HonzonTreasuryPool: AccountId = PalletId(*b"aca/hztp").into_account_truncating();
	pub HomaTreasuryPool: AccountId = PalletId(*b"aca/hmtp").into_account_truncating();
	// Incentive reward Pools
	pub HonzonInsuranceRewardPool: AccountId = PalletId(*b"aca/hirp").into_account_truncating();
	pub HonzonLiquitationRewardPool: AccountId = PalletId(*b"aca/hlrp").into_account_truncating();
	pub StakingRewardPool: AccountId = PalletId(*b"aca/strp").into_account_truncating();
	pub CollatorsRewardPool: AccountId = PalletId(*b"aca/clrp").into_account_truncating();
	pub EcosystemRewardPool: AccountId = PalletId(*b"aca/esrp").into_account_truncating();

	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![vec![GetStakingCurrencyId::get()]];
}

impl fees::Config for Runtime {
	type Event = Event;
	type UpdateOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type Currency = Balances;
	type Currencies = Currencies;
	type NativeCurrencyId = GetNativeCurrencyId;
	type AllocationPeriod = ConstU64<10>;
	type DEX = DEX;
	type DexSwapJointList = AlternativeSwapPathJointList;
	type WeightInfo = ();
}

parameter_types! {
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub EnabledTradingPairs: Vec<TradingPair> = vec![
		TradingPair::from_currency_ids(AUSD, ACA).unwrap(),
		TradingPair::from_currency_ids(AUSD, DOT).unwrap(),
	];
	pub const TradingPathLimit: u32 = 4;
}

impl module_dex::Config for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type PalletId = DEXPalletId;
	type Erc20InfoMapping = ();
	type DEXIncentives = ();
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type ExtendedProvisioningBlocks = ConstU64<0>;
	type OnLiquidityPoolUpdated = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: module_currencies,
		Fees: fees,
		DEX: module_dex,
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

		let native_currency_id = ACA;

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == native_currency_id)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != native_currency_id)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		fees::GenesisConfig::<Runtime> {
			incomes: vec![
				(
					IncomeSource::TxFee,
					vec![(NetworkTreasuryPool::get(), 80), (CollatorsRewardPool::get(), 20)],
				),
				(IncomeSource::XcmFee, vec![(NetworkTreasuryPool::get(), 100)]),
			],
			treasuries: vec![(
				NetworkTreasuryPool::get(),
				100,
				vec![
					(StakingRewardPool::get(), 80),
					(CollatorsRewardPool::get(), 10),
					(EcosystemRewardPool::get(), 10),
				],
			)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_dex::GenesisConfig::<Runtime> {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			initial_added_liquidity_pools: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
