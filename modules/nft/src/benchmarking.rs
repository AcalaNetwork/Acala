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

//! Benchmarks for the nft module.

#![cfg(feature = "runtime-benchmarks")]

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{AccountIdConversion, StaticLookup, UniqueSaturatedInto};

pub use crate::*;
use orml_traits::BasicCurrencyExtended;
use primitives::Balance;

pub struct Module<T: Config>(crate::Pallet<T>);

pub trait Config: crate::Config + orml_nft::Config + pallet_proxy::Config + module_currencies::Config {}

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

benchmarks! {
	// create NFT class
	create_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let base_currency_amount = dollar(1000);

		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;
	}: _(RawOrigin::Signed(caller), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))

	// mint NFT token
	mint {
		let i in 1 .. 1000;

		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to);

		let base_currency_amount = dollar(1000);
		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Pallet::<T>::next_class_id());
		crate::Pallet::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as module_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
	}: _(RawOrigin::Signed(module_account), to_lookup, 0u32.into(), vec![1], i)

	// transfer NFT token to another account
	transfer {
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller_lookup = T::Lookup::unlookup(caller.clone());
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let base_currency_amount = dollar(1000);
		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Pallet::<T>::next_class_id());
		crate::Pallet::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as module_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
		crate::Pallet::<T>::mint(RawOrigin::Signed(module_account).into(), to_lookup, 0u32.into(), vec![1], 1)?;
	}: _(RawOrigin::Signed(to), caller_lookup, (0u32.into(), 0u32.into()))

	// burn NFT token
	burn {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let base_currency_amount = dollar(1000);
		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Pallet::<T>::next_class_id());
		crate::Pallet::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as module_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
		crate::Pallet::<T>::mint(RawOrigin::Signed(module_account).into(), to_lookup, 0u32.into(), vec![1], 1)?;
	}: _(RawOrigin::Signed(to), (0u32.into(), 0u32.into()))

	// destroy NFT class
	destroy_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to);

		let base_currency_amount = dollar(1000);

		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Pallet::<T>::next_class_id());
		crate::Pallet::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
	}: _(RawOrigin::Signed(module_account), 0u32.into(), to_lookup)
}

#[cfg(test)]
mod mock {
	use super::*;

	use codec::{Decode, Encode};
	use frame_support::{
		parameter_types,
		traits::{Filter, InstanceFilter},
		weights::Weight,
		RuntimeDebug,
	};
	use orml_traits::parameter_type_with_key;
	use primitives::{evm::EvmAddress, mocks::MockAddressMapping, Amount, BlockNumber, CurrencyId};
	use sp_core::{crypto::AccountId32, H256};
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
		DispatchError, DispatchResult, ModuleId, Perbill,
	};
	use support::{EVMBridge, InvokeContext};

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: Weight = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}

	pub type AccountId = AccountId32;

	impl frame_system::Config for Runtime {
		type BaseCallFilter = BaseFilter;
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Call = Call;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type BlockHashCount = BlockHashCount;
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
	}
	parameter_types! {
		pub const ExistentialDeposit: u64 = 1;
	}
	impl pallet_balances::Config for Runtime {
		type Balance = Balance;
		type Event = ();
		type DustRemoval = ();
		type ExistentialDeposit = ExistentialDeposit;
		type AccountStore = frame_system::Pallet<Runtime>;
		type MaxLocks = ();
		type WeightInfo = ();
	}
	impl pallet_utility::Config for Runtime {
		type Event = ();
		type Call = Call;
		type WeightInfo = ();
	}
	parameter_types! {
		pub const ProxyDepositBase: u64 = 1;
		pub const ProxyDepositFactor: u64 = 1;
		pub const MaxProxies: u16 = 4;
		pub const MaxPending: u32 = 2;
		pub const AnnouncementDepositBase: u64 = 1;
		pub const AnnouncementDepositFactor: u64 = 1;
	}
	#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
	pub enum ProxyType {
		Any,
		JustTransfer,
		JustUtility,
	}
	impl Default for ProxyType {
		fn default() -> Self {
			Self::Any
		}
	}
	impl InstanceFilter<Call> for ProxyType {
		fn filter(&self, c: &Call) -> bool {
			match self {
				ProxyType::Any => true,
				ProxyType::JustTransfer => matches!(c, Call::Balances(pallet_balances::Call::transfer(..))),
				ProxyType::JustUtility => matches!(c, Call::Utility(..)),
			}
		}
		fn is_superset(&self, o: &Self) -> bool {
			self == &ProxyType::Any || self == o
		}
	}
	pub struct BaseFilter;
	impl Filter<Call> for BaseFilter {
		fn filter(c: &Call) -> bool {
			match *c {
				// Remark is used as a no-op call in the benchmarking
				Call::System(SystemCall::remark(_)) => true,
				Call::System(_) => false,
				_ => true,
			}
		}
	}
	impl pallet_proxy::Config for Runtime {
		type Event = ();
		type Call = Call;
		type Currency = Balances;
		type ProxyType = ProxyType;
		type ProxyDepositBase = ProxyDepositBase;
		type ProxyDepositFactor = ProxyDepositFactor;
		type MaxProxies = MaxProxies;
		type WeightInfo = ();
		type CallHasher = BlakeTwo256;
		type MaxPending = MaxPending;
		type AnnouncementDepositBase = AnnouncementDepositBase;
		type AnnouncementDepositFactor = AnnouncementDepositFactor;
	}

	pub type NativeCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

	parameter_type_with_key! {
		pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
			Default::default()
		};
	}

	impl orml_tokens::Config for Runtime {
		type Event = ();
		type Balance = Balance;
		type Amount = Amount;
		type CurrencyId = CurrencyId;
		type WeightInfo = ();
		type ExistentialDeposits = ExistentialDeposits;
		type OnDust = ();
	}

	pub struct MockEVMBridge;
	impl<AccountId, Balance> EVMBridge<AccountId, Balance> for MockEVMBridge
	where
		AccountId: Default,
		Balance: Default,
	{
		fn total_supply(_context: InvokeContext) -> Result<Balance, DispatchError> {
			Ok(Default::default())
		}

		fn balance_of(_context: InvokeContext, _address: EvmAddress) -> Result<Balance, DispatchError> {
			Ok(Default::default())
		}

		fn transfer(_context: InvokeContext, _to: EvmAddress, _value: Balance) -> DispatchResult {
			Ok(())
		}

		fn get_origin() -> Option<AccountId> {
			None
		}

		fn set_origin(_origin: AccountId) {}
	}

	impl module_currencies::Config for Runtime {
		type Event = ();
		type MultiCurrency = Tokens;
		type NativeCurrency = NativeCurrency;
		type WeightInfo = ();
		type AddressMapping = MockAddressMapping;
		type EVMBridge = MockEVMBridge;
	}

	parameter_types! {
		pub const CreateClassDeposit: Balance = 200;
		pub const CreateTokenDeposit: Balance = 100;
		pub const NftModuleId: ModuleId = ModuleId(*b"aca/aNFT");
	}
	impl crate::Config for Runtime {
		type Event = ();
		type CreateClassDeposit = CreateClassDeposit;
		type CreateTokenDeposit = CreateTokenDeposit;
		type ModuleId = NftModuleId;
		type Currency = NativeCurrency;
		type WeightInfo = ();
	}

	impl orml_nft::Config for Runtime {
		type ClassId = u32;
		type TokenId = u64;
		type ClassData = ClassData;
		type TokenData = TokenData;
	}

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
	type Block = frame_system::mocking::MockBlock<Runtime>;

	frame_support::construct_runtime!(
		pub enum Runtime where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			Utility: pallet_utility::{Pallet, Call, Event},
			Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
			Currencies: module_currencies::{Pallet, Call, Event<T>},
			Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
			Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>},
			OrmlNFT: orml_nft::{Pallet, Storage, Config<T>},
			NFT: crate::{Pallet, Call, Event<T>},
		}
	);

	use frame_system::Call as SystemCall;

	impl crate::Config for Runtime {}

	pub fn new_test_ext() -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn test_create_class() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_create_class::<Runtime>());
		});
	}

	#[test]
	fn test_mint() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_mint::<Runtime>());
		});
	}

	#[test]
	fn test_transfer() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer::<Runtime>());
		});
	}

	#[test]
	fn test_burn() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_burn::<Runtime>());
		});
	}

	#[test]
	fn test_destroy_class() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_destroy_class::<Runtime>());
		});
	}
}
