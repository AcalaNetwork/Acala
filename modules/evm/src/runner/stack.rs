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

//! EVM stack-based runner.
// Synchronize with https://github.com/paritytech/frontier/blob/master/frame/evm/src/runner/stack.rs

use crate::{
	precompiles::PrecompileSet,
	runner::{
		state::{StackExecutor, StackSubstateMetadata},
		Runner as RunnerT, StackState as StackStateT,
	},
	AccountInfo, AccountStorages, Accounts, BalanceOf, CallInfo, Config, CreateInfo, Error, ExecutionInfo, One, Pallet,
	STORAGE_SIZE,
};
use frame_support::{
	dispatch::DispatchError,
	ensure, log,
	traits::{Currency, ExistenceRequirement, Get},
	transactional,
};
use module_evm_utiltity::{
	ethereum::Log,
	evm::{self, backend::Backend as BackendT, ExitError, ExitReason, Transfer},
};
use module_support::AddressMapping;
pub use primitives::{
	convert_decimals_from_evm,
	evm::{EvmAddress, Vicinity, MIRRORED_NFT_ADDRESS_START},
	ReserveIdentifier,
};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{UniqueSaturatedInto, Zero};
use sp_std::{boxed::Box, collections::btree_set::BTreeSet, marker::PhantomData, mem, vec, vec::Vec};

#[derive(Default)]
pub struct Runner<T: Config> {
	_marker: PhantomData<T>,
}

impl<T: Config> Runner<T> {
	/// Execute an EVM operation.
	pub fn execute<'config, F, R>(
		source: H160,
		origin: H160,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &'config evm::Config,
		f: F,
	) -> Result<ExecutionInfo<R>, sp_runtime::DispatchError>
	where
		F: FnOnce(&mut StackExecutor<'config, SubstrateStackState<'_, 'config, T>>) -> (ExitReason, R),
	{
		let gas_price = U256::one();
		let vicinity = Vicinity {
			gas_price,
			origin,
			..Default::default()
		};

		let metadata = StackSubstateMetadata::new(gas_limit, storage_limit, config);
		let state = SubstrateStackState::new(&vicinity, metadata);
		let mut executor = StackExecutor::new_with_precompile(state, config, T::Precompiles::execute);

		// NOTE: charge from transaction-payment
		// let total_fee = gas_price
		// 	.checked_mul(U256::from(gas_limit))
		// 	.ok_or(Error::<T>::FeeOverflow)?;
		// let total_payment = value.checked_add(total_fee).ok_or(Error::<T>::PaymentOverflow)?;
		// let source_account = Pallet::<T>::account_basic(&source);
		// ensure!(source_account.balance >= total_payment, Error::<T>::BalanceLow);

		// Deduct fee from the `source` account.
		// let fee = T::ChargeTransactionPayment::withdraw_fee(&source, total_fee)?;
		ensure!(
			convert_decimals_from_evm(value.low_u128()).is_some(),
			Error::<T>::InvalidDecimals
		);

		if !config.estimate {
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
		let actual_fee = executor.fee(gas_price);
		log::debug!(
			target: "evm",
			"Execution {:?} [source: {:?}, value: {}, gas_limit: {}, actual_fee: {}]",
			reason,
			source,
			value,
			gas_limit,
			actual_fee
		);

		// NOTE: refund from transaction-payment
		// Refund fees to the `source` account if deducted more before,
		// T::OnChargeTransaction::correct_and_deposit_fee(&source, actual_fee, fee)?;

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
			"Storage logs: {:?}",
			state.substate.storage_logs
		);
		let mut sum_storage: i32 = 0;
		for (target, storage) in &state.substate.storage_logs {
			if !config.estimate {
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
			sum_storage += storage;
		}
		if actual_storage != sum_storage {
			log::debug!(
				target: "evm",
				"ChargeStorageFailed [actual_storage: {:?}, sum_storage: {:?}]",
				actual_storage, sum_storage
			);
			return Err(Error::<T>::ChargeStorageFailed.into());
		}

		if !config.estimate {
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
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError> {
		// if the contract not published, the caller must be developer or contract or maintainer.
		// if the contract not exists, let evm try to execute it and handle the error.
		ensure!(
			Pallet::<T>::can_call_contract(&target, &source),
			Error::<T>::NoPermission
		);

		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(source, origin, value, gas_limit, storage_limit, config, |executor| {
			// TODO: EIP-2930
			executor.transact_call(source, target, value, input, gas_limit, vec![])
		})
	}

	/// Require transactional here. Always need to send events.
	#[transactional]
	fn create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(source, source, value, gas_limit, storage_limit, config, |executor| {
			let address = executor
				.create_address(evm::CreateScheme::Legacy { caller: source })
				.unwrap_or_default(); // transact_create will check the address
			(
				// TODO: EIP-2930
				executor.transact_create(source, value, init, gas_limit, vec![]),
				address,
			)
		})
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
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		let code_hash = H256::from_slice(Keccak256::digest(&init).as_slice());
		Self::execute(source, source, value, gas_limit, storage_limit, config, |executor| {
			let address = executor
				.create_address(evm::CreateScheme::Create2 {
					caller: source,
					code_hash,
					salt,
				})
				.unwrap_or_default(); // transact_create2 will check the address
			(
				// TODO: EIP-2930
				executor.transact_create2(source, value, init, salt, gas_limit, vec![]),
				address,
			)
		})
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
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		let value = U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(value));
		Self::execute(source, source, value, gas_limit, storage_limit, config, |executor| {
			(
				// TODO: EIP-2930
				executor.transact_create_at_address(source, address, value, init, gas_limit, vec![]),
				address,
			)
		})
	}
}

struct SubstrateStackSubstate<'config> {
	metadata: StackSubstateMetadata<'config>,
	deletes: BTreeSet<H160>,
	logs: Vec<Log>,
	storage_logs: Vec<(H160, i32)>,
	parent: Option<Box<SubstrateStackSubstate<'config>>>,
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
			},
			_marker: PhantomData,
		}
	}
}

impl<'vicinity, 'config, T: Config> BackendT for SubstrateStackState<'vicinity, 'config, T> {
	fn gas_price(&self) -> U256 {
		self.vicinity.gas_price
	}
	fn origin(&self) -> H160 {
		self.vicinity.origin
	}

	fn block_hash(&self, number: U256) -> H256 {
		if number > U256::from(u32::max_value()) {
			H256::default()
		} else {
			let number = T::BlockNumber::from(number.as_u32());
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
		U256::from(T::ChainId::get())
	}

	#[cfg(feature = "evm-tests")]
	fn exists(&self, address: H160) -> bool {
		Accounts::<T>::contains_key(&address)
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
		AccountStorages::<T>::get(&address, index)
	}

	fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
		Some(self.storage(address, index))
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

	fn is_cold(&self, _address: H160) -> bool {
		// TODO: EIP-2930
		false
	}

	fn is_storage_cold(&self, _address: H160, _key: H256) -> bool {
		// TODO: EIP-2930
		false
	}

	fn inc_nonce(&mut self, address: H160) {
		Accounts::<T>::mutate(&address, |maybe_account| {
			if let Some(account) = maybe_account.as_mut() {
				account.nonce += One::one()
			} else {
				let mut account_info = <AccountInfo<T::Index>>::new(Default::default(), None);
				account_info.nonce += One::one();
				*maybe_account = Some(account_info);
			}
		});
	}

	fn set_storage(&mut self, address: H160, index: H256, value: H256) {
		if value == H256::default() {
			log::debug!(
				target: "evm",
				"Removing storage for {:?} [index: {:?}]",
				address,
				index,
			);
			<AccountStorages<T>>::remove(address, index);
			Pallet::<T>::update_contract_storage_size(&address, -(STORAGE_SIZE as i32));
			self.substate.metadata.storage_meter_mut().refund(STORAGE_SIZE);
		} else {
			log::debug!(
				target: "evm",
				"Updating storage for {:?} [index: {:?}, value: {:?}]",
				address,
				index,
				value,
			);
			<AccountStorages<T>>::insert(address, index, value);
			Pallet::<T>::update_contract_storage_size(&address, STORAGE_SIZE as i32);
			self.substate.metadata.storage_meter_mut().charge(STORAGE_SIZE);
		}
	}

	fn reset_storage(&mut self, address: H160) {
		<AccountStorages<T>>::remove_prefix(address, None);
	}

	fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
		self.substate.log(address, topics, data)
	}

	fn set_deleted(&mut self, address: H160) {
		self.substate.set_deleted(address);
	}

	fn set_code(&mut self, address: H160, code: Vec<u8>) {
		log::debug!(
			target: "evm",
			"Inserting code ({} bytes) at {:?}",
			code.len(),
			address
		);

		let caller: H160;
		let mut substate = &self.substate;

		loop {
			// get maintainer from parent caller
			// `enter_substate` will do `spit_child`
			if substate.parent.is_none() {
				log::error!(
					target: "evm",
					"get parent's maintainer failed. address: {:?}",
					address
				);
				debug_assert!(false);
				return;
			}

			substate = substate.parent.as_ref().expect("has checked; qed");

			if let Some(c) = substate.metadata().caller() {
				// the caller maybe is contract and not published.
				// get the parent's maintainer.
				if !Pallet::<T>::is_account_empty(c) {
					caller = *c;
					break;
				}
			}
		}

		log::debug!(
			target: "evm",
			"set_code: address: {:?}, maintainer: {:?}",
			address,
			caller
		);

		let code_size = code.len() as u32;
		Pallet::<T>::create_contract(caller, address, code);

		let used_storage = code_size.saturating_add(T::NewContractExtraBytes::get());
		Pallet::<T>::update_contract_storage_size(&address, used_storage as i32);
		self.substate.metadata.storage_meter_mut().charge(used_storage);
	}

	fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
		if transfer.value.is_zero() {
			return Ok(());
		}
		let source = T::AddressMapping::get_account_id(&transfer.source);
		let target = T::AddressMapping::get_account_id(&transfer.target);
		let amount = convert_decimals_from_evm(transfer.value.low_u128())
			.ok_or(ExitError::Other(Into::<&str>::into(Error::<T>::InvalidDecimals).into()))?
			.unique_saturated_into();

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
	}
}
