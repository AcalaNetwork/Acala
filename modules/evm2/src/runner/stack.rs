//! EVM stack-based runner.

use crate::precompiles::PrecompileSet;
use crate::runner::Runner as RunnerT;
use crate::{
	AccountInfo,
	/*	AccountCodes, AccountStorages, AddressMapping, BlockHashMapping, Config, Error, Event,
	 *	FeeCalculator, OnChargeEVMTransaction, Pallet, PrecompileSet, */
	Accounts,
	One,
};
use crate::{AccountStorages, CallInfo, Config, CreateInfo, Error, Event, ExecutionInfo, Pallet};
use evm::backend::Backend as BackendT;
use evm::executor::{StackExecutor, StackState as StackStateT, StackSubstateMetadata};
use evm::{ExitError, ExitReason, Transfer};
use frame_support::{
	ensure, log,
	traits::{Currency, ExistenceRequirement, Get},
};
use module_support::AddressMapping;
pub use primitives::{
	evm::{Account, EvmAddress, Log, Vicinity},
	ReserveIdentifier, MIRRORED_NFT_ADDRESS_START,
};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::{boxed::Box, collections::btree_set::BTreeSet, marker::PhantomData, mem, vec::Vec};

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
	) -> Result<ExecutionInfo<R>, Error<T>>
	where
		F: FnOnce(&mut StackExecutor<'config, SubstrateStackState<'_, 'config, T>>) -> (ExitReason, R),
	{
		let gas_price = U256::one();
		let vicinity = Vicinity { gas_price, origin };

		let metadata = StackSubstateMetadata::new(gas_limit, &config);
		let state = SubstrateStackState::new(&vicinity, metadata);
		let mut executor = StackExecutor::new_with_precompile(state, config, T::Precompiles::execute);

		let total_fee = gas_price
			.checked_mul(U256::from(gas_limit))
			.ok_or(Error::<T>::FeeOverflow)?;
		let total_payment = value.checked_add(total_fee).ok_or(Error::<T>::PaymentOverflow)?;
		let source_account = Pallet::<T>::account_basic(&source);
		ensure!(source_account.balance >= total_payment, Error::<T>::BalanceLow);

		// Deduct fee from the `source` account.
		//TODO
		//let fee = T::ChargeTransactionPayment::withdraw_fee(&source, total_fee)?;

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

		// Refund fees to the `source` account if deducted more before,
		//TODO
		//T::OnChargeTransaction::correct_and_deposit_fee(&source, actual_fee, fee)?;

		let state = executor.into_state();

		for address in state.substate.deletes {
			log::debug!(
				target: "evm",
				"Deleting account at {:?}",
				address
			);
			Pallet::<T>::remove_account(&address);
		}

		for log in &state.substate.logs {
			log::trace!(
				target: "evm",
				"Inserting log for {:?}, topics ({}) {:?}, data ({}): {:?}]",
				log.address,
				log.topics.len(),
				log.topics,
				log.data.len(),
				log.data
			);
			Pallet::<T>::deposit_event(Event::<T>::Log(Log {
				address: log.address,
				topics: log.topics.clone(),
				data: log.data.clone(),
			}));
		}

		Ok(ExecutionInfo {
			value: retv,
			exit_reason: reason,
			used_gas,
			used_storage: 0,
			logs: state.substate.logs,
		})
	}
}

impl<T: Config> RunnerT<T> for Runner<T> {
	type Error = Error<T>;

	fn call(
		source: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CallInfo, Self::Error> {
		// if the contract not deployed, the caller must be developer or contract or maintainer.
		// if the contract not exists, let evm try to execute it and handle the error.
		ensure!(
			Pallet::<T>::can_call_contract(&target, &source),
			Error::<T>::NoPermission
		);

		Self::execute(source, origin, value, gas_limit, storage_limit, config, |executor| {
			executor.transact_call(source, target, value, input, gas_limit)
		})
	}

	fn create(
		source: H160,
		init: Vec<u8>,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, Self::Error> {
		Self::execute(source, source, value, gas_limit, storage_limit, config, |executor| {
			let address = executor.create_address(evm::CreateScheme::Legacy { caller: source });
			(executor.transact_create(source, value, init, gas_limit), address)
		})
	}

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: U256,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, Self::Error> {
		let code_hash = H256::from_slice(Keccak256::digest(&init).as_slice());
		Self::execute(source, source, value, gas_limit, storage_limit, config, |executor| {
			let address = executor.create_address(evm::CreateScheme::Create2 {
				caller: source,
				code_hash,
				salt,
			});
			(executor.transact_create2(source, value, init, salt, gas_limit), address)
		})
	}

	fn create_at_address(
		source: H160,
		init: Vec<u8>,
		value: U256,
		assigned_address: H160,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, Self::Error> {
		let code_hash = H256::from_slice(Keccak256::digest(&init).as_slice());
		Self::execute(source, source, value, gas_limit, storage_limit, config, |executor| {
			(
				executor.transact_create(source, value, init, gas_limit),
				assigned_address,
			)
		})
	}
}

struct SubstrateStackSubstate<'config> {
	metadata: StackSubstateMetadata<'config>,
	deletes: BTreeSet<H160>,
	logs: Vec<Log>,
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
		};
		mem::swap(&mut entering, self);

		self.parent = Some(Box::new(entering));

		sp_io::storage::start_transaction();
	}

	pub fn exit_commit(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot commit on root substate");
		mem::swap(&mut exited, self);

		self.metadata.swallow_commit(exited.metadata)?;
		self.logs.append(&mut exited.logs);
		self.deletes.append(&mut exited.deletes);

		sp_io::storage::commit_transaction();
		Ok(())
	}

	pub fn exit_revert(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot discard on root substate");
		mem::swap(&mut exited, self);
		self.metadata.swallow_revert(exited.metadata)?;

		sp_io::storage::rollback_transaction();
		Ok(())
	}

	pub fn exit_discard(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot discard on root substate");
		mem::swap(&mut exited, self);
		self.metadata.swallow_discard(exited.metadata)?;

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
	// save all callers from `inc_nonce` to get maintainer
	callers: Vec<H160>,
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
				parent: None,
			},
			callers: vec![],
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
		Pallet::<T>::find_author()
	}

	fn block_timestamp(&self) -> U256 {
		let now: u128 = pallet_timestamp::Pallet::<T>::get().unique_saturated_into();
		U256::from(now / 1000)
	}

	fn block_difficulty(&self) -> U256 {
		U256::zero()
	}

	fn block_gas_limit(&self) -> U256 {
		U256::zero()
	}

	fn chain_id(&self) -> U256 {
		U256::from(T::ChainId::get())
	}

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
		let addr = Pallet::<T>::handle_mirrored_token(address);
		Pallet::<T>::code_at_address(&addr).into_inner()
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		<AccountStorages<T>>::get(address, index)
	}

	fn original_storage(&self, _address: H160, _index: H256) -> Option<H256> {
		None
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

	fn inc_nonce(&mut self, address: H160) {
		Accounts::<T>::mutate(&address, |maybe_account| {
			if let Some(account) = maybe_account.as_mut() {
				account.nonce += One::one()
			} else {
				let mut account_info = <AccountInfo<T>>::new(Default::default(), None);
				account_info.nonce += One::one();
				*maybe_account = Some(account_info);
			}
		});

		self.callers.push(address);
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
		} else {
			log::debug!(
				target: "evm",
				"Updating storage for {:?} [index: {:?}, value: {:?}]",
				address,
				index,
				value,
			);
			<AccountStorages<T>>::insert(address, index, value);
		}
	}

	fn reset_storage(&mut self, address: H160) {
		<AccountStorages<T>>::remove_prefix(address, None);
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

		// avoid config.create_increase_nonce
		// should not default value
		let caller = self.callers.pop().map_or(Default::default(), |v| {
			if v != address {
				v
			} else {
				self.callers.pop().unwrap_or_default()
			}
		});
		Pallet::<T>::create_account(address, caller, code);
	}

	fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
		let source = T::AddressMapping::get_account_id(&transfer.source);
		let target = T::AddressMapping::get_account_id(&transfer.target);

		T::Currency::transfer(
			&source,
			&target,
			transfer.value.low_u128().unique_saturated_into(),
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| ExitError::OutOfFund)
	}

	fn reset_balance(&mut self, _address: H160) {
		// Do nothing on reset balance in Substrate.
		//
		// This function exists in EVM because a design issue
		// (arguably a bug) in SELFDESTRUCT that can cause total
		// issurance to be reduced. We do not need to replicate this.
	}

	fn touch(&mut self, _address: H160) {
		// Do nothing on touch in Substrate.
		//
		// EVM pallet considers all accounts to exist, and distinguish
		// only empty and non-empty accounts. This avoids many of the
		// subtle issues in EIP-161.
	}
}
