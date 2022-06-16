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

//! Benchmarks for the nft module.

#![cfg(feature = "runtime-benchmarks")]

use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::{dispatch::DispatchErrorWithPostInfo, traits::Get, weights::DispatchClass};
use frame_system::RawOrigin;
use sp_runtime::traits::{AccountIdConversion, StaticLookup, UniqueSaturatedInto};
use sp_std::collections::btree_map::BTreeMap;

pub use crate::*;
use primitives::Balance;

pub struct Module<T: Config>(crate::Pallet<T>);

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn test_attr() -> Attributes {
	let mut attr: Attributes = BTreeMap::new();
	for i in 0..30 {
		attr.insert(vec![i], vec![0; 64]);
	}
	attr
}

fn create_token_class<T: Config>(caller: T::AccountId) -> Result<T::AccountId, DispatchErrorWithPostInfo> {
	let base_currency_amount = dollar(1000);
	<T as module::Config>::Currency::make_free_balance_be(&caller, base_currency_amount.unique_saturated_into());

	let module_account: T::AccountId =
		T::PalletId::get().into_sub_account_truncating(orml_nft::Pallet::<T>::next_class_id());
	crate::Pallet::<T>::create_class(
		RawOrigin::Signed(caller).into(),
		vec![1],
		Properties(
			ClassProperty::Transferable
				| ClassProperty::Burnable
				| ClassProperty::Mintable
				| ClassProperty::ClassPropertiesMutable,
		),
		test_attr(),
	)?;

	<T as module::Config>::Currency::make_free_balance_be(
		&module_account,
		base_currency_amount.unique_saturated_into(),
	);

	Ok(module_account)
}

benchmarks! {
	// create NFT class
	create_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let base_currency_amount = dollar(1000);

		<T as module::Config>::Currency::make_free_balance_be(&caller, base_currency_amount.unique_saturated_into());
	}: _(RawOrigin::Signed(caller), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable), test_attr())

	// mint NFT token
	mint {
		let i in 1 .. 1000;

		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to);

		let module_account = create_token_class::<T>(caller)?;
	}: _(RawOrigin::Signed(module_account), to_lookup, 0u32.into(), vec![1], test_attr(), i)

	// transfer NFT token to another account
	transfer {
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller_lookup = T::Lookup::unlookup(caller.clone());
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let module_account = create_token_class::<T>(caller)?;

		crate::Pallet::<T>::mint(RawOrigin::Signed(module_account).into(), to_lookup, 0u32.into(), vec![1], test_attr(), 1)?;
	}: _(RawOrigin::Signed(to), caller_lookup, (0u32.into(), 0u32.into()))

	// burn NFT token
	burn {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let module_account = create_token_class::<T>(caller)?;

		crate::Pallet::<T>::mint(RawOrigin::Signed(module_account).into(), to_lookup, 0u32.into(), vec![1], test_attr(), 1)?;
	}: _(RawOrigin::Signed(to), (0u32.into(), 0u32.into()))

	// burn NFT token with remark
	burn_with_remark {
		let b in 0 .. *T::BlockLength::get().max.get(DispatchClass::Normal) as u32;
		let remark_message = vec![1; b as usize];
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let module_account = create_token_class::<T>(caller)?;

		crate::Pallet::<T>::mint(RawOrigin::Signed(module_account).into(), to_lookup, 0u32.into(), vec![1], test_attr(), 1)?;
	}: _(RawOrigin::Signed(to), (0u32.into(), 0u32.into()), remark_message)

	// destroy NFT class
	destroy_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		let base_currency_amount = dollar(1000);

		let module_account = create_token_class::<T>(caller)?;

	}: _(RawOrigin::Signed(module_account), 0u32.into(), caller_lookup)

	update_class_properties {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to);

		let module_account = create_token_class::<T>(caller)?;
	}: _(RawOrigin::Signed(module_account), 0u32.into(), Properties(ClassProperty::Transferable.into()))
}

#[cfg(test)]
mod mock {
	use super::*;
	use crate as nft;

	use codec::{Decode, Encode};
	use frame_support::{
		parameter_types,
		traits::{ConstU128, ConstU32, ConstU64, Contains, InstanceFilter},
		PalletId, RuntimeDebug,
	};
	use sp_core::{crypto::AccountId32, H256};
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
	};

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
		type Event = ();
		type DustRemoval = ();
		type ExistentialDeposit = ConstU128<1>;
		type AccountStore = frame_system::Pallet<Runtime>;
		type MaxLocks = ();
		type MaxReserves = ConstU32<50>;
		type ReserveIdentifier = ReserveIdentifier;
		type WeightInfo = ();
	}
	impl pallet_utility::Config for Runtime {
		type Event = ();
		type Call = Call;
		type PalletsOrigin = OriginCaller;
		type WeightInfo = ();
	}
	#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
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
				ProxyType::JustTransfer => matches!(c, Call::Balances(pallet_balances::Call::transfer { .. })),
				ProxyType::JustUtility => matches!(c, Call::Utility(..)),
			}
		}
		fn is_superset(&self, o: &Self) -> bool {
			self == &ProxyType::Any || self == o
		}
	}
	pub struct BaseFilter;
	impl Contains<Call> for BaseFilter {
		fn contains(c: &Call) -> bool {
			match *c {
				// Remark is used as a no-op call in the benchmarking
				Call::System(SystemCall::remark { .. }) => true,
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
		type ProxyDepositBase = ConstU128<1>;
		type ProxyDepositFactor = ConstU128<1>;
		type MaxProxies = ConstU32<4>;
		type WeightInfo = ();
		type CallHasher = BlakeTwo256;
		type MaxPending = ConstU32<2>;
		type AnnouncementDepositBase = ConstU128<1>;
		type AnnouncementDepositFactor = ConstU128<1>;
	}

	parameter_types! {
		pub const NftPalletId: PalletId = PalletId(*b"aca/aNFT");
	}

	impl crate::Config for Runtime {
		type Event = ();
		type Currency = Balances;
		type CreateClassDeposit = ConstU128<200>;
		type CreateTokenDeposit = ConstU128<100>;
		type DataDepositPerByte = ConstU128<10>;
		type PalletId = NftPalletId;
		type MaxAttributesBytes = ConstU32<2048>;
		type WeightInfo = ();
	}

	impl orml_nft::Config for Runtime {
		type ClassId = u32;
		type TokenId = u64;
		type ClassData = ClassData<Balance>;
		type TokenData = TokenData<Balance>;
		type MaxClassMetadata = ConstU32<1024>;
		type MaxTokenMetadata = ConstU32<1024>;
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
			Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>},
			OrmlNFT: orml_nft::{Pallet, Storage, Config<T>},
			NFT: nft::{Pallet, Call, Event<T>},
		}
	);

	use frame_system::Call as SystemCall;

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
	use super::mock::*;
	use super::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::new_test_ext(), super::Runtime,);
}
