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

//! EVM stack-based runner.
// Synchronize with https://github.com/paritytech/frontier/blob/bcae569524/frame/evm/src/runner/stack.rs

use crate::{
	runner::{
		state::{Accessed, StackExecutor, StackState as StackStateT, StackSubstateMetadata},
		Runner as RunnerT, RunnerExtended,
	},
	AccountStorages, BalanceOf, CallInfo, Config, CreateInfo, Error, ExecutionInfo, Pallet, STORAGE_SIZE,
};
use frame_support::{
	ensure,
	traits::{Currency, ExistenceRequirement, Get},
	transactional,
};
use frame_system::pallet_prelude::*;
use module_evm_utility::{
	ethereum::Log,
	evm::{self, backend::Backend as BackendT, ExitError, ExitReason, Transfer},
};
use module_support::{AddressMapping, EVMManager, EVM};
pub use primitives::{
	evm::{convert_decimals_from_evm, EvmAddress, Vicinity, MIRRORED_NFT_ADDRESS_START},
	ReserveIdentifier,
};
use sp_core::{defer, H160, H256, U256};
use sp_runtime::{
	traits::{UniqueSaturatedInto, Zero},
	DispatchError,
};
use sp_std::{
	boxed::Box,
	collections::{btree_map::BTreeMap, btree_set::BTreeSet},
	marker::PhantomData,
	mem,
	vec::Vec,
};

#[derive(Default)]
pub struct Runner<T: Config> {
	_marker: PhantomData<T>,
}

impl<T: Config> Runner<T> {
	/// Execute an EVM operation.
	pub fn execute<'config, 'precompiles, F, R>(
		source: H160,
		origin: H160,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &'config evm::Config,
		skip_storage_rent: bool,
		precompiles: &'precompiles T::PrecompilesType,
		f: F,
	) -> Result<ExecutionInfo<R>, sp_runtime::DispatchError>
	where
		F: FnOnce(
			&mut StackExecutor<'config, 'precompiles, SubstrateStackState<'_, 'config, T>, T::PrecompilesType>,
		) -> (ExitReason, R),
	{
		let gas_price = U256::one();
		let vicinity = Vicinity {
			gas_price,
			origin,
			..Default::default()
		};

		let metadata = StackSubstateMetadata::new(gas_limit, storage_limit, config);
		let state = SubstrateStackState::new(&vicinity, metadata);
		let mut executor = StackExecutor::new_with_precompiles(state, config, precompiles);

		ensure!(
			convert_decimals_from_evm(
				TryInto::<BalanceOf<T>>::try_into(value).map_err(|_| Error::<T>::InvalidDecimals)?
			)
			.is_some(),
			Error::<T>::InvalidDecimals
		);

		if !skip_storage_rent {
			Pallet::<T>::reserve_storage(&origin, storage_limit).map_err(|e| {
				log::debug!(
					target: "evm",
					"ReserveStorageFailed {:?} [source: {:?}, storage_limit: {:?}]",
					e,
					origin,
					storage_limit
				);
				Error::<T>::ReserveStorageFailed
			})?;
		}

		// Execute the EVM call.
		let (reason, retv) = f(&mut executor);

		let used_gas = U256::from(executor.used_gas());
		log::debug!(
			target: "evm",
			"Execution {:?} [source: {:?}, value: {}, gas_limit: {}, used_gas: {}]",
			reason,
			source,
			value,
			gas_limit,
			used_gas,
		);

		let state = executor.into_state();

		// charge storage
		let actual_storage = state
			.metadata()
			.storage_meter()
			.finish()
			.ok_or(Error::<T>::OutOfStorage)?;
		let used_storage = state.metadata().storage_meter().total_used();
		let refunded_storage = state.metadata().storage_meter().total_refunded();
		log::debug!(
			target: "evm",
			"Storage limit: {:?}, actual storage: {:?}, used storage: {:?}, refunded storage: {:?}, storage logs: {:?}",
			state.metadata().storage_meter().storage_limit(),
			actual_storage,
			used_storage,
			refunded_storage,
			state.substate.storage_logs
		);
		let mut sum_storage: i32 = 0;
		for (target, storage) in &state.substate.storage_logs.into_iter().fold(
			BTreeMap::<H160, i32>::new(),
			|mut bmap, (target, storage)| {
				bmap.entry(target)
					.and_modify(|x| *x = x.saturating_add(storage))
					.or_insert(storage);
				bmap
			},
		) {
			if !skip_storage_rent {
				Pallet::<T>::charge_storage(&origin, target, *storage).map_err(|e| {
					log::debug!(
						target: "evm",
						"ChargeStorageFailed {:?} [source: {:?}, target: {:?}, storage: {:?}]",
						e,
						origin,
						target,
						storage
					);
					Error::<T>::ChargeStorageFailed
				})?;
			}
			sum_storage = sum_storage.saturating_add(*storage);
		}
		if actual_storage != sum_storage {
			log::debug!(
				target: "evm",
				"ChargeStorageFailed [actual_storage: {:?}, sum_storage: {:?}]",
				actual_storage, sum_storage
			);
			return Err(Error::<T>::ChargeStorageFailed.into());
		}

		if !skip_storage_rent {
			Pallet::<T>::unreserve_storage(&origin, storage_limit, used_storage, refunded_storage).map_err(|e| {
				log::debug!(
					target: "evm",
					"UnreserveStorageFailed {:?} [source: {:?}, storage_limit: {:?}, used_storage: {:?}, refunded_storage: {:?}]",
					e,
					origin,
					storage_limit,
					used_storage,
					refunded_storage
				);
				Error::<T>::UnreserveStorageFailed
			})?;
		}

		for address in state.substate.deletes {
			log::debug!(
				target: "evm",
				"Deleting account at {:?}",
				address
			);
			Pallet::<T>::remove_contract(&origin, &address).map_err(|e| {
				log::debug!(
					target: "evm",
					"CannotKillContract address {:?}, reason: {:?}",
					address,
					e
				);
				Error::<T>::CannotKillContract
			})?;
		}

		log::debug!(
			target: "evm",
			"Execution logs {:?}",
			state.substate.logs
		);

		Ok(ExecutionInfo {
			value: retv,
			exit_reason: reason,
			used_gas,
			used_storage: actual_storage,
			logs: state.substate.logs,
		})
	}
}

impl<T: Config> RunnerT<T> for Runner<T> {
	/// Require transactional here. Always need to send events.
	#[transactional]
	fn call(
		source: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError> {
		// if the contract not published, the caller must be developer or contract or maintainer.
		// if the contract not exists, let evm try to execute it and handle the error.
		ensure!(
			Pallet::<T>::can_call_contract(&target, &source),
			Error::<T>::NoPermission
		);

		let precompiles = T::PrecompilesValue::get();
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(
			source,
			origin,
			value,
			gas_limit,
			storage_limit,
			config,
			false,
			&precompiles,
			|executor| executor.transact_call(source, target, value, input, gas_limit, access_list),
		)
	}

	/// Require transactional here. Always need to send events.
	#[transactional]
	fn create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		let precompiles = T::PrecompilesValue::get();
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(
			source,
			source,
			value,
			gas_limit,
			storage_limit,
			config,
			false,
			&precompiles,
			|executor| {
				let address = executor
					.create_address(evm::CreateScheme::Legacy { caller: source })
					.unwrap_or_default(); // transact_create will check the address
				let (reason, _) = executor.transact_create(source, value, init, gas_limit, access_list);
				(reason, address)
			},
		)
	}

	/// Require transactional here. Always need to send events.
	#[transactional]
	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		let precompiles = T::PrecompilesValue::get();
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		let code_hash = H256::from(sp_io::hashing::keccak_256(&init));
		Self::execute(
			source,
			source,
			value,
			gas_limit,
			storage_limit,
			config,
			false,
			&precompiles,
			|executor| {
				let address = executor
					.create_address(evm::CreateScheme::Create2 {
						caller: source,
						code_hash,
						salt,
					})
					.unwrap_or_default(); // transact_create2 will check the address
				let (reason, _) = executor.transact_create2(source, value, init, salt, gas_limit, access_list);
				(reason, address)
			},
		)
	}

	/// Require transactional here. Always need to send events.
	#[transactional]
	fn create_at_address(
		source: H160,
		address: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		let precompiles = T::PrecompilesValue::get();
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(
			source,
			source,
			value,
			gas_limit,
			storage_limit,
			config,
			false,
			&precompiles,
			|executor| {
				let (reason, _) =
					executor.transact_create_at_address(source, address, value, init, gas_limit, access_list);
				(reason, address)
			},
		)
	}
}

impl<T: Config> RunnerExtended<T> for Runner<T> {
	/// Special method for rpc call which won't charge for storage rent
	/// Same as call but with skip_storage_rent: true
	fn rpc_call(
		source: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError> {
		// Ensure eth_call has evm origin, otherwise xcm charge rent fee will fail.
		Pallet::<T>::set_origin(T::AddressMapping::get_account_id(&origin));
		defer!(Pallet::<T>::kill_origin());

		let precompiles = T::PrecompilesValue::get();
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(
			source,
			origin,
			value,
			gas_limit,
			storage_limit,
			config,
			true,
			&precompiles,
			|executor| executor.transact_call(source, target, value, input, gas_limit, access_list),
		)
	}

	/// Special method for rpc create which won't charge for storage rent
	/// Same as create but with skip_storage_rent: true
	fn rpc_create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		let precompiles = T::PrecompilesValue::get();
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(
			source,
			source,
			value,
			gas_limit,
			storage_limit,
			config,
			true,
			&precompiles,
			|executor| {
				let address = executor
					.create_address(evm::CreateScheme::Legacy { caller: source })
					.unwrap_or_default(); // transact_create will check the address
				let (reason, _) = executor.transact_create(source, value, init, gas_limit, access_list);
				(reason, address)
			},
		)
	}
}

struct SubstrateStackSubstate<'config> {
	metadata: StackSubstateMetadata<'config>,
	deletes: BTreeSet<H160>,
	logs: Vec<Log>,
	storage_logs: Vec<(H160, i32)>,
	parent: Option<Box<SubstrateStackSubstate<'config>>>,
	known_original_storage: BTreeMap<(H160, H256), H256>,
}

impl<'config> SubstrateStackSubstate<'config> {
	pub fn metadata(&self) -> &StackSubstateMetadata<'config> {
		&self.metadata
	}

	pub fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
		&mut self.metadata
	}

	pub fn enter(&mut self, gas_limit: u64, is_static: bool) {
		let mut entering = Self {
			metadata: self.metadata.spit_child(gas_limit, is_static),
			parent: None,
			deletes: BTreeSet::new(),
			logs: Vec::new(),
			storage_logs: Vec::new(),
			known_original_storage: BTreeMap::new(),
		};
		mem::swap(&mut entering, self);

		self.parent = Some(Box::new(entering));

		sp_io::storage::start_transaction();
	}

	pub fn exit_commit(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot commit on root substate");
		mem::swap(&mut exited, self);

		let target = self.metadata().target().expect("Storage target is none");
		let storage = exited.metadata().storage_meter().used_storage();

		self.metadata.swallow_commit(exited.metadata).map_err(|e| {
			sp_io::storage::rollback_transaction();
			e
		})?;
		self.logs.append(&mut exited.logs);
		self.deletes.append(&mut exited.deletes);

		exited.storage_logs.push((target, storage));
		self.storage_logs.append(&mut exited.storage_logs);

		sp_io::storage::commit_transaction();
		Ok(())
	}

	pub fn exit_revert(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot discard on root substate");
		mem::swap(&mut exited, self);
		self.metadata.swallow_revert(exited.metadata).map_err(|e| {
			sp_io::storage::rollback_transaction();
			e
		})?;

		sp_io::storage::rollback_transaction();
		Ok(())
	}

	pub fn exit_discard(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot discard on root substate");
		mem::swap(&mut exited, self);
		self.metadata.swallow_discard(exited.metadata).map_err(|e| {
			sp_io::storage::rollback_transaction();
			e
		})?;

		sp_io::storage::rollback_transaction();
		Ok(())
	}

	pub fn deleted(&self, address: H160) -> bool {
		if self.deletes.contains(&address) {
			return true;
		}

		if let Some(parent) = self.parent.as_ref() {
			return parent.deleted(address);
		}

		false
	}

	pub fn set_deleted(&mut self, address: H160) {
		self.deletes.insert(address);
	}

	pub fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
		self.logs.push(Log { address, topics, data });
	}

	fn recursive_is_cold<F: Fn(&Accessed) -> bool>(&self, f: &F) -> bool {
		let local_is_accessed = self.metadata.accessed().as_ref().map(f).unwrap_or(false);
		if local_is_accessed {
			false
		} else {
			self.parent.as_ref().map(|p| p.recursive_is_cold(f)).unwrap_or(true)
		}
	}

	pub fn known_original_storage(&self, address: H160, index: H256) -> Option<H256> {
		if let Some(parent) = self.parent.as_ref() {
			return parent.known_original_storage(address, index);
		}
		self.known_original_storage.get(&(address, index)).copied()
	}

	pub fn set_known_original_storage(&mut self, address: H160, index: H256, value: H256) {
		if let Some(ref mut parent) = self.parent {
			return parent.set_known_original_storage(address, index, value);
		}
		self.known_original_storage.insert((address, index), value);
	}
}

#[cfg(feature = "evm-tests")]
impl<'config> SubstrateStackSubstate<'config> {
	pub fn mark_account_dirty(&self, address: H160) {
		// https://github.com/ethereum/go-ethereum/blob/v1.10.16/core/state/state_object.go#L143
		// insert in parent to make sure it doesn't get discarded
		if address == H160::from_low_u64_be(3) {
			if let Some(parent) = self.parent.as_ref() {
				return parent.mark_account_dirty(address);
			}
		}
		self.metadata().dirty_accounts.borrow_mut().insert(address);
	}

	pub fn is_account_dirty(&self, address: H160) -> bool {
		if self.metadata().dirty_accounts.borrow().contains(&address) {
			return true;
		}
		if let Some(parent) = self.parent.as_ref() {
			return parent.is_account_dirty(address);
		}
		false
	}
}

/// Substrate backend for EVM.
pub struct SubstrateStackState<'vicinity, 'config, T> {
	vicinity: &'vicinity Vicinity,
	substate: SubstrateStackSubstate<'config>,
	_marker: PhantomData<T>,
}

impl<'vicinity, 'config, T: Config> SubstrateStackState<'vicinity, 'config, T> {
	/// Create a new backend with given vicinity.
	pub fn new(vicinity: &'vicinity Vicinity, metadata: StackSubstateMetadata<'config>) -> Self {
		Self {
			vicinity,
			substate: SubstrateStackSubstate {
				metadata,
				deletes: BTreeSet::new(),
				logs: Vec::new(),
				storage_logs: Vec::new(),
				parent: None,
				known_original_storage: BTreeMap::new(),
			},
			_marker: PhantomData,
		}
	}
}

#[cfg(feature = "evm-tests")]
impl<'vicinity, 'config, T: Config> SubstrateStackState<'vicinity, 'config, T> {
	pub fn deleted_accounts(&self) -> Vec<H160> {
		self.substate.deletes.iter().copied().collect()
	}

	pub fn empty_accounts(&self) -> Vec<H160> {
		self.metadata()
			.dirty_accounts
			.borrow()
			.iter()
			.filter(|x| self.is_empty(**x))
			.copied()
			.collect()
	}
}

impl<'vicinity, 'config, T: Config> BackendT for SubstrateStackState<'vicinity, 'config, T> {
	fn gas_price(&self) -> U256 {
		self.vicinity.gas_price
	}
	fn origin(&self) -> H160 {
		self.vicinity.origin
	}

	#[cfg(feature = "evm-tests")]
	fn block_randomness(&self) -> Option<H256> {
		self.vicinity.block_randomness
	}

	#[cfg(not(feature = "evm-tests"))]
	fn block_randomness(&self) -> Option<H256> {
		Some(self.vicinity.block_randomness.unwrap_or(Pallet::<T>::get_randomness()))
	}

	fn block_hash(&self, number: U256) -> H256 {
		if number > U256::from(u32::MAX) {
			H256::default()
		} else {
			let number = BlockNumberFor::<T>::from(number.as_u32());
			H256::from_slice(frame_system::Pallet::<T>::block_hash(number).as_ref())
		}
	}

	fn block_number(&self) -> U256 {
		let number: u128 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
		U256::from(number)
	}

	fn block_coinbase(&self) -> H160 {
		self.vicinity.block_coinbase.unwrap_or(Pallet::<T>::find_author())
	}

	fn block_timestamp(&self) -> U256 {
		let now: u128 = pallet_timestamp::Pallet::<T>::get().unique_saturated_into();
		U256::from(now / 1000)
	}

	fn block_difficulty(&self) -> U256 {
		self.vicinity.block_difficulty.unwrap_or_default()
	}

	fn block_gas_limit(&self) -> U256 {
		self.vicinity.block_gas_limit.unwrap_or_default()
	}

	fn chain_id(&self) -> U256 {
		U256::from(Pallet::<T>::chain_id())
	}

	#[cfg(feature = "evm-tests")]
	fn exists(&self, address: H160) -> bool {
		crate::Accounts::<T>::contains_key(&address) || self.substate.is_account_dirty(address)
	}

	#[cfg(not(feature = "evm-tests"))]
	fn exists(&self, _address: H160) -> bool {
		true
	}

	fn basic(&self, address: H160) -> evm::backend::Basic {
		let account = Pallet::<T>::account_basic(&address);

		evm::backend::Basic {
			balance: account.balance,
			nonce: account.nonce,
		}
	}

	fn code(&self, address: H160) -> Vec<u8> {
		Pallet::<T>::code_at_address(&address).into_inner()
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		AccountStorages::<T>::get(address, index)
	}

	fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
		if let Some(value) = self.substate.known_original_storage(address, index) {
			Some(value)
		} else {
			Some(self.storage(address, index))
		}
	}

	fn block_base_fee_per_gas(&self) -> sp_core::U256 {
		self.vicinity.block_base_fee_per_gas.unwrap_or(U256::one())
	}
}

impl<'vicinity, 'config, T: Config> StackStateT<'config> for SubstrateStackState<'vicinity, 'config, T> {
	fn metadata(&self) -> &StackSubstateMetadata<'config> {
		self.substate.metadata()
	}

	fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
		self.substate.metadata_mut()
	}

	fn enter(&mut self, gas_limit: u64, is_static: bool) {
		self.substate.enter(gas_limit, is_static)
	}

	fn exit_commit(&mut self) -> Result<(), ExitError> {
		self.substate.exit_commit()
	}

	fn exit_revert(&mut self) -> Result<(), ExitError> {
		self.substate.exit_revert()
	}

	fn exit_discard(&mut self) -> Result<(), ExitError> {
		self.substate.exit_discard()
	}

	fn is_empty(&self, address: H160) -> bool {
		Pallet::<T>::is_account_empty(&address)
	}

	fn deleted(&self, address: H160) -> bool {
		self.substate.deleted(address)
	}

	fn inc_nonce(&mut self, address: H160) -> Result<(), ExitError> {
		Pallet::<T>::inc_nonce(&address);
		Ok(())
	}

	fn set_storage(&mut self, address: H160, index: H256, value: H256) {
		let current = <AccountStorages<T>>::get(address, index);

		// keep track of original storage
		if self.substate.known_original_storage(address, index).is_none() {
			self.substate.set_known_original_storage(address, index, current);
		};

		if value == H256::default() {
			log::debug!(
				target: "evm",
				"Removing storage for {:?} [index: {:?}]",
				address,
				index,
			);
			<AccountStorages<T>>::remove(address, index);

			// storage meter
			if !current.is_zero() {
				Pallet::<T>::update_contract_storage_size(&address, -(STORAGE_SIZE as i32));
				self.substate.metadata.storage_meter_mut().refund(STORAGE_SIZE);
			}
		} else {
			log::debug!(
				target: "evm",
				"Updating storage for {:?} [index: {:?}, value: {:?}]",
				address,
				index,
				value,
			);
			<AccountStorages<T>>::insert(address, index, value);

			// storage meter
			if current.is_zero() {
				Pallet::<T>::update_contract_storage_size(&address, STORAGE_SIZE as i32);
				self.substate.metadata.storage_meter_mut().charge(STORAGE_SIZE);
			}
		}
	}

	fn reset_storage(&mut self, address: H160) {
		// use drain_prefix to avoid wasm-bencher counting limit as write operation
		<AccountStorages<T>>::drain_prefix(address).for_each(drop);
	}

	fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
		self.substate.log(address, topics, data)
	}

	fn set_deleted(&mut self, address: H160) {
		self.substate.set_deleted(address)
	}

	fn set_code(&mut self, address: H160, code: Vec<u8>) {
		log::debug!(
			target: "evm",
			"Inserting code ({} bytes) at {:?}",
			code.len(),
			address
		);

		// get maintainer from parent caller `enter_substate` will do `spit_child`
		let parent = match self.substate.parent {
			Some(ref parent) => parent,
			None => {
				log::error!(
					target: "evm",
					"get parent's maintainer failed. address: {:?}",
					address
				);
				debug_assert!(false);
				return;
			}
		};

		let caller = match parent.metadata().caller() {
			Some(ref caller) => caller,
			None => {
				log::error!(
					target: "evm",
					"get parent's caller failed. address: {:?}",
					address
				);
				debug_assert!(false);
				return;
			}
		};

		let is_published = self.substate.metadata.origin_code_address().map_or_else(
			|| {
				// contracts are published if deployer is not in developer mode
				let is_developer = Pallet::<T>::query_developer_status(&T::AddressMapping::get_account_id(caller));
				!is_developer
			},
			|addr| {
				// inherent the published status from origin code address
				Pallet::<T>::accounts(addr)
					.map_or(false, |account| account.contract_info.map_or(false, |v| v.published))
			},
		);

		log::debug!(
			target: "evm",
			"set_code: address: {:?}, maintainer: {:?}, publish: {:?}",
			address,
			caller,
			is_published
		);

		let code_size = code.len() as u32;
		Pallet::<T>::create_contract(*caller, address, is_published, code);

		let used_storage = code_size.saturating_add(T::NewContractExtraBytes::get());
		Pallet::<T>::update_contract_storage_size(&address, used_storage as i32);
		self.substate.metadata.storage_meter_mut().charge(used_storage);
	}

	fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
		self.touch(transfer.target);
		if transfer.value.is_zero() {
			return Ok(());
		}
		let source = T::AddressMapping::get_account_id(&transfer.source);
		let target = T::AddressMapping::get_account_id(&transfer.target);
		let amount = convert_decimals_from_evm(
			TryInto::<BalanceOf<T>>::try_into(transfer.value).map_err(|_| ExitError::OutOfFund)?,
		)
		.ok_or(ExitError::Other(Into::<&str>::into(Error::<T>::InvalidDecimals).into()))?;

		log::debug!(
			target: "evm",
			"transfer [source: {:?}, target: {:?}, amount: {:?}]",
			source,
			target,
			amount
		);

		if T::Currency::free_balance(&source) < amount {
			return Err(ExitError::OutOfFund);
		}

		T::Currency::transfer(&source, &target, amount, ExistenceRequirement::AllowDeath)
			.map_err(|e| ExitError::Other(Into::<&str>::into(e).into()))
	}

	fn reset_balance(&mut self, address: H160) {
		// Address and target can be the same during SELFDESTRUCT. In that case we transfer the
		// remaining balance to treasury
		let source = T::AddressMapping::get_account_id(&address);
		let balance = T::Currency::free_balance(&source);
		if !balance.is_zero() {
			if let Err(e) = T::Currency::transfer(
				&source,
				&T::TreasuryAccount::get(),
				balance,
				ExistenceRequirement::AllowDeath,
			) {
				debug_assert!(
					false,
					"Failed to transfer remaining balance to treasury with error: {:?}",
					e
				);
			}
		}
	}

	fn touch(&mut self, _address: H160) {
		// Do nothing on touch in Substrate.
		//
		// EVM pallet considers all accounts to exist, and distinguish
		// only empty and non-empty accounts. This avoids many of the
		// subtle issues in EIP-161.

		// this is needed only for evm-tests to keep track of dirty accounts
		#[cfg(feature = "evm-tests")]
		self.substate.mark_account_dirty(_address);
	}

	fn is_cold(&self, address: H160) -> bool {
		self.substate
			.recursive_is_cold(&|a| a.accessed_addresses.contains(&address))
	}

	fn is_storage_cold(&self, address: H160, key: H256) -> bool {
		self.substate
			.recursive_is_cold(&|a: &Accessed| a.accessed_storage.contains(&(address, key)))
	}

	fn code_size(&self, address: H160) -> U256 {
		Pallet::<T>::code_size_at_address(&address)
	}

	fn code_hash(&self, address: H160) -> H256 {
		Pallet::<T>::code_hash_at_address(&address)
	}
}
