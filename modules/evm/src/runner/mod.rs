pub mod handler;
pub mod storage_meter;

use crate::{AddressMapping, BalanceOf, CallInfo, Config, CreateInfo, Error, Pallet, Vicinity};
use evm::{CreateScheme, ExitError, ExitReason};
use evm_runtime::Handler as HandlerT;
use frame_support::{
	debug,
	traits::{Currency, ExistenceRequirement, Get},
};
use handler::Handler;
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use sp_runtime::{traits::Zero, DispatchError, DispatchResult, SaturatedConversion, TransactionOutcome};
use sp_std::{marker::PhantomData, vec::Vec};

#[derive(Default)]
pub struct Runner<T: Config> {
	_marker: PhantomData<T>,
}

impl<T: Config> Runner<T> {
	fn inner_create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		assigned_address: Option<H160>,
		salt: Option<H256>,
		tag: &'static str,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		debug::debug!(
			target: "evm",
			"{:?}: source {:?}, gas_limit: {:?}, storage_limit: {:?}",
			tag,
			source,
			gas_limit,
			storage_limit,
		);

		let vicinity = Vicinity {
			gas_price: U256::one(),
			origin: source,
		};

		let address = if let Some(addr) = assigned_address {
			addr
		} else {
			let scheme = if let Some(s) = salt {
				let code_hash = H256::from_slice(Keccak256::digest(&init).as_slice());
				CreateScheme::Create2 {
					caller: source,
					code_hash,
					salt: s,
				}
			} else {
				CreateScheme::Legacy { caller: source }
			};
			Handler::<T>::create_address(scheme)
		};

		Handler::<T>::inc_nonce(source);

		Handler::<T>::run_transaction(
			&vicinity,
			gas_limit,
			storage_limit,
			address,
			false,
			config,
			|substate| {
				if let Err(e) = Self::transfer(source, address, value) {
					return TransactionOutcome::Rollback(Err(e));
				}

				let (reason, out) = substate.execute(
					source,
					address,
					U256::from(value.saturated_into::<u128>()),
					init,
					Vec::new(),
				);

				let mut create_info = CreateInfo {
					exit_reason: reason.clone(),
					address,
					output: Vec::default(),
					used_gas: U256::from(substate.used_gas()),
					used_storage: substate.used_storage(),
				};

				debug::debug!(
					target: "evm",
					"{:?}-result: create_info {:?}",
					tag,
					create_info
				);

				if !reason.is_succeed() {
					create_info.output = out;
					return TransactionOutcome::Rollback(Ok(create_info));
				}

				if let Err(e) = substate.gasometer.record_deposit(out.len()) {
					create_info.exit_reason = e.into();
					return TransactionOutcome::Rollback(Ok(create_info));
				}

				create_info.used_gas = U256::from(substate.used_gas());

				Handler::<T>::inc_nonce(address);

				if substate
					.storage_meter
					.charge((out.len() as u32).saturating_add(T::NewContractExtraBytes::get()))
					.is_err()
				{
					create_info.exit_reason = ExitReason::Error(ExitError::OutOfGas);
					return TransactionOutcome::Rollback(Ok(create_info));
				}

				create_info.used_storage = substate.used_storage();

				if let Err(e) = <Pallet<T>>::on_contract_initialization(&address, &source, out) {
					create_info.exit_reason = e.into();
					return TransactionOutcome::Rollback(Ok(create_info));
				}

				TransactionOutcome::Commit(Ok(create_info))
			},
		)?
	}

	fn transfer(source: H160, target: H160, value: BalanceOf<T>) -> DispatchResult {
		if value.is_zero() {
			return Ok(());
		}

		let from = T::AddressMapping::get_account_id(&source);
		let to = T::AddressMapping::get_account_id(&target);
		T::Currency::transfer(&from, &to, value, ExistenceRequirement::AllowDeath)
	}
}

impl<T: Config> Runner<T> {
	pub fn call(
		sender: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError> {
		debug::debug!(
			target: "evm",
			"call: sender:{:?}, origin: {:?}, target: {:?}, input: {:?}, gas_limit: {:?}, storage_limit: {:?}",
			sender,
			origin,
			target,
			input,
			gas_limit,
			storage_limit,
		);

		let vicinity = Vicinity {
			gas_price: U256::one(),
			origin,
		};

		// if the contract not deployed, the caller must be developer or contract.
		// if the contract not exists, let evm try to execute it and handle the error.
		if Handler::<T>::is_undeployed_contract(&target) && !Handler::<T>::has_permission_to_call(&sender) {
			return Err(Error::<T>::NoPermission.into());
		}

		Handler::<T>::inc_nonce(sender);

		Handler::<T>::run_transaction(&vicinity, gas_limit, storage_limit, target, false, config, |substate| {
			if let Err(e) = Self::transfer(sender, target, value) {
				return TransactionOutcome::Rollback(Err(e));
			}

			let code = substate.code(target);
			let (reason, out) =
				substate.execute(sender, target, U256::from(value.saturated_into::<u128>()), code, input);

			let call_info = CallInfo {
				exit_reason: reason.clone(),
				output: out,
				used_gas: U256::from(substate.used_gas()),
				used_storage: substate.used_storage(),
			};

			debug::debug!(
				target: "evm",
				"call-result: call_info {:?}",
				call_info
			);

			if !reason.is_succeed() {
				return TransactionOutcome::Rollback(Ok(call_info));
			}

			TransactionOutcome::Commit(Ok(call_info))
		})?
	}

	pub fn create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		Self::inner_create(
			source,
			init,
			value,
			gas_limit,
			storage_limit,
			None,
			None,
			"create",
			config,
		)
	}

	pub fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		Self::inner_create(
			source,
			init,
			value,
			gas_limit,
			storage_limit,
			None,
			Some(salt),
			"create2",
			config,
		)
	}

	pub fn create_at_address(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		assigned_address: H160,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		Self::inner_create(
			source,
			init,
			value,
			gas_limit,
			storage_limit,
			Some(assigned_address),
			None,
			"create-system-contract",
			config,
		)
	}
}
