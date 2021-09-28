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

//! Benchmarks for the Homa Lite module.

#![cfg(feature = "runtime-benchmarks")]

pub use crate::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;

pub struct Module<T: Config>(crate::Pallet<T>);

const SEED: u32 = 0;

benchmarks! {
	on_idle {
		let amount = 1_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller1: T::AccountId = account("callera", 0, SEED);
		let caller2: T::AccountId = account("callerb", 0, SEED);
		let caller3: T::AccountId = account("callerc", 0, SEED);
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller1, amount)?;
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller2, amount)?;
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller3, amount)?;
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller1).into(), amount, Permill::default());
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller2.clone()).into(), amount, Permill::default());
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller3.clone()).into(), amount, Permill::default());
		let _ = crate::Pallet::<T>::schedule_unbond(RawOrigin::Root.into(), amount*2, <T as frame_system::Config>::BlockNumber::default());
	}: {
		let _ = crate::Pallet::<T>::on_idle(<T as frame_system::Config>::BlockNumber::default(), 1_000_000_000);
	}

	mint {
		let amount = 1_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		<T as module::Config>::Currency::deposit(T::StakingCurrencyId::get(), &caller, amount)?;
		let _ = crate::Pallet::<T>::set_minting_cap(RawOrigin::Root.into(), amount)?;
	}: _(RawOrigin::Signed(caller), amount)

	mint_for_requests {
		let amount = 1_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller1: T::AccountId = account("callera", 0, SEED);
		let caller2: T::AccountId = account("callerb", 0, SEED);
		let caller3: T::AccountId = account("callerc", 0, SEED);
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller1, amount)?;
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller2, amount)?;
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller3, amount)?;
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller1).into(), amount, Permill::default());
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller2.clone()).into(), amount, Permill::default());
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller3.clone()).into(), amount, Permill::default());

		<T as module::Config>::Currency::deposit(T::StakingCurrencyId::get(), &caller, amount*2)?;
		crate::Pallet::<T>::set_minting_cap(RawOrigin::Root.into(), amount*2)?;
	}: _(RawOrigin::Signed(caller), amount*2, vec![caller2, caller3])

	set_total_staking_currency {}: _(RawOrigin::Root, 1_000_000_000_000)

	adjust_total_staking_currency {}: _(RawOrigin::Root, AmountOf::<T>::default())

	set_minting_cap {
	}: _(RawOrigin::Root, 1_000_000_000_000_000_000)

	set_xcm_base_weight {
	}: _(RawOrigin::Root, 1_000_000_000)

	request_redeem {
		let amount = 1_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller, amount)?;
	}: _(RawOrigin::Signed(caller), amount, Permill::default())

	schedule_unbond {}: _(RawOrigin::Root, 1_000_000_000_000, <T as frame_system::Config>::BlockNumber::default())

	replace_schedule_unbond {}: _(RawOrigin::Root, vec![(1_000_000, <T as frame_system::Config>::BlockNumber::default()), (1_000_000_000, <T as frame_system::Config>::BlockNumber::default())])
}

#[cfg(test)]
mod benchmark_mock {
	use super::*;
	type AccountId = AccountId32;
	type BlockNumber = u64;
	use crate as module_homa_lite;
	use frame_support::{ord_parameter_types, parameter_types};
	use frame_system::EnsureRoot;
	use mock::{MockXcm, ACALA, KSM, LKSM, MOCK_XCM_ACCOUNTID, MOCK_XCM_DESTINATION, PARACHAIN_ID, ROOT};
	use module_support::mocks::MockAddressMapping;
	use orml_traits::parameter_type_with_key;
	use primitives::Amount;
	use sp_core::H256;
	use sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32};

	mod homa_lite {
		pub use super::super::*;
	}

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
	}

	impl frame_system::Config for Runtime {
		type BaseCallFilter = ();
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
		type BlockHashCount = BlockHashCount;
		type DbWeight = ();
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = pallet_balances::AccountData<Balance>;
		type OnNewAccount = ();
		type OnKilledAccount = ();
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
		type DustRemovalWhitelist = ();
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

	pub type AdaptedBasicCurrency =
		module_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

	parameter_types! {
		pub const GetNativeCurrencyId: CurrencyId = ACALA;
	}

	impl module_currencies::Config for Runtime {
		type Event = Event;
		type MultiCurrency = Tokens;
		type NativeCurrency = AdaptedBasicCurrency;
		type GetNativeCurrencyId = GetNativeCurrencyId;
		type WeightInfo = ();
		type AddressMapping = MockAddressMapping;
		type EVMBridge = ();
	}

	parameter_types! {
		pub const StakingCurrencyId: CurrencyId = KSM;
		pub const LiquidCurrencyId: CurrencyId = LKSM;
		pub MinimumMintThreshold: Balance = millicent(1);
		pub const MockXcmDestination: MultiLocation = MOCK_XCM_DESTINATION;
		pub const MockXcmAccountId: AccountId = MOCK_XCM_ACCOUNTID;
		pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(10, 1);
		pub const MaxRewardPerEra: Permill = Permill::from_percent(1);
		pub MintFee: Balance = millicent(1000);
		pub BaseWithdrawFee: Permill = Permill::from_rational(1u32, 1_000u32); // 0.1%
		pub const ParachainAccount: AccountId = ROOT;
		pub const MaximumRedeemRequestMatchesForMint: u32 = 2;
		pub static MockRelayBlockNumberProvider: u64 = 0;
		pub const RelaychainUnbondingSlashingSpans: u32 = 5;
		pub const SubAccountIndex: u16 = 0;
		pub const ParachainId: ParaId = ParaId::from(PARACHAIN_ID);
		pub XcmUnbondFee: Balance = dollar(1);
	}
	ord_parameter_types! {
		pub const Root: AccountId = ROOT;
	}

	impl Config for Runtime {
		type Event = Event;
		type WeightInfo = ();
		type Currency = Currencies;
		type StakingCurrencyId = StakingCurrencyId;
		type LiquidCurrencyId = LiquidCurrencyId;
		type GovernanceOrigin = EnsureSignedBy<Root, AccountId>;
		type MinimumMintThreshold = MinimumMintThreshold;
		type XcmTransfer = MockXcm;
		type SovereignSubAccountLocation = MockXcmDestination;
		type SubAccountIndex = SubAccountIndex;
		type DefaultExchangeRate = DefaultExchangeRate;
		type MaxRewardPerEra = MaxRewardPerEra;
		type MintFee = MintFee;
		type XcmSender = MockXcm;
		type RelaychainCallBuilder = RelaychainCallBuilder<Runtime, ParachainId>;
		type BaseWithdrawFee = BaseWithdrawFee;
		type XcmUnbondFee = XcmUnbondFee;
		type RelaychainBlockNumber = MockRelayBlockNumberProvider;
		type ParachainAccount = ParachainAccount;
		type MaximumRedeemRequestMatchesForMint = MaximumRedeemRequestMatchesForMint;
		type RelaychainUnbondingSlashingSpans = RelaychainUnbondingSlashingSpans;
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
			HomaLite: module_homa_lite::{Pallet, Call, Storage, Event<T>},
			PalletBalances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
			Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
			Currencies: module_currencies::{Pallet, Call, Event<T>},
		}
	);

	pub struct ExtBuilder;

	impl Default for ExtBuilder {
		fn default() -> Self {
			ExtBuilder {}
		}
	}

	impl ExtBuilder {
		pub fn build(self) -> sp_io::TestExternalities {
			let t = frame_system::GenesisConfig::default()
				.build_storage::<Runtime>()
				.unwrap();

			let mut ext = sp_io::TestExternalities::new(t);
			ext.execute_with(|| System::set_block_number(1));
			ext
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use benchmark_mock::*;
	use frame_support::assert_ok;

	#[test]
	fn test_mint() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_mint());
		});
	}
	#[test]
	fn test_mint_for_requests() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_mint_for_requests());
		});
	}
	#[test]
	fn test_set_total_staking_currency() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_set_total_staking_currency());
		});
	}
	#[test]
	fn test_adjust_total_staking_currency() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_adjust_total_staking_currency());
		});
	}
	#[test]
	fn test_set_minting_cap() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_set_minting_cap());
		});
	}
	#[test]
	fn test_set_xcm_base_weight() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_set_xcm_base_weight());
		});
	}
	#[test]
	fn test_request_redeem() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_request_redeem());
		});
	}
	#[test]
	fn test_schedule_unbond() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_schedule_unbond());
		});
	}
	#[test]
	fn test_replace_schedule_unbond() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_replace_schedule_unbond());
		});
	}
}
