pub mod handler;
pub mod storage_meter;

use crate::{
	precompiles::Precompiles, AddressMapping, BalanceOf, CallInfo, Config, CreateInfo, Error, Module, Vicinity,
};
use evm::{CreateScheme, ExitError, ExitReason};
use evm_runtime::Handler as HandlerT;
use frame_support::{
	debug,
	traits::{BalanceStatus, Currency, ExistenceRequirement, Get, ReservableCurrency},
};
use handler::Handler;
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::{
	traits::{Saturating, Zero},
	DispatchError, DispatchResult, SaturatedConversion, TransactionOutcome,
};
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
		gas_limit: u32,
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

		let mut substate = Handler::<T>::new_with_precompile(
			&vicinity,
			gas_limit as usize,
			storage_limit,
			false,
			config,
			T::Precompiles::execute,
		);

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
			substate.create_address(scheme)
		};

		substate.inc_nonce(source);

		frame_support::storage::with_transaction(|| {
			if let Err(e) = Self::transfer(source, address, value) {
				return TransactionOutcome::Rollback(Err(e));
			}

			if let Err(e) = Self::deduct_storage(source, address, storage_limit) {
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
				used_storage: U256::from(substate.used_storage()),
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

			substate.inc_nonce(address);

			if let Err(e) = <Module<T>>::on_contract_initialization(&address, &source, out, None) {
				create_info.exit_reason = e.into();
				TransactionOutcome::Rollback(Ok(create_info))
			} else {
				let storage_usage = Module::<T>::storage_usage(address);
				if let Err(e) = substate.storagemeter.record_cost(storage_usage) {
					create_info.exit_reason = e.into();
					return TransactionOutcome::Rollback(Ok(create_info));
				}
				if let Err(e) = Self::refund_storage(source, address, substate.storagemeter.storage()) {
					debug::debug!(
						target: "evm",
						"create-result: refund_storage {:?}",
						e
					);
					create_info.exit_reason = ExitReason::Error(ExitError::Other("RefundedStorageFailed".into()));
					return TransactionOutcome::Rollback(Ok(create_info));
				}

				create_info.used_storage = U256::from(substate.used_storage());
				TransactionOutcome::Commit(Ok(create_info))
			}
		})
	}

	fn transfer(source: H160, target: H160, value: BalanceOf<T>) -> Result<(), DispatchError> {
		if value.is_zero() {
			return Ok(());
		}

		let from = T::AddressMapping::get_account_id(&source);
		let to = T::AddressMapping::get_account_id(&target);
		T::Currency::transfer(&from, &to, value, ExistenceRequirement::AllowDeath)
	}

	fn deduct_storage(source: H160, target: H160, used_storage: u32) -> Result<(), DispatchError> {
		if used_storage.is_zero() {
			return Ok(());
		}

		let from = T::AddressMapping::get_account_id(&source);
		let to = T::AddressMapping::get_account_id(&target);

		let amount = T::StorageDepositPerByte::get().saturating_mul((used_storage as u32).into());
		T::Currency::transfer(&from, &to, amount, ExistenceRequirement::AllowDeath)?;
		T::Currency::reserve(&to, amount)
	}

	fn refund_storage(source: H160, target: H160, refunded_storage: u32) -> Result<(), DispatchError> {
		if refunded_storage.is_zero() {
			return Ok(());
		}

		let from = T::AddressMapping::get_account_id(&target);
		let to = T::AddressMapping::get_account_id(&source);

		let amount = T::StorageDepositPerByte::get().saturating_mul((refunded_storage as u32).into());
		T::Currency::repatriate_reserved(&from, &to, amount, BalanceStatus::Free)?;
		Ok(())
	}
}

impl<T: Config> Runner<T> {
	pub fn call(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u32,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError> {
		debug::debug!(
			target: "evm",
			"call: source {:?}, target: {:?}, input: {:?}, gas_limit: {:?}, storage_limit: {:?}",
			source,
			target,
			input,
			gas_limit,
			storage_limit,
		);

		let vicinity = Vicinity {
			gas_price: U256::one(),
			origin: source,
		};

		let mut substate = Handler::<T>::new_with_precompile(
			&vicinity,
			gas_limit as usize,
			storage_limit,
			false,
			config,
			T::Precompiles::execute,
		);

		// if the contract not deployed, the caller must be developer or contract.
		substate.is_undeployed_contract(&target).map_or(
			Err(Error::<T>::NoPermission.into()),
			|is_undeployed| -> DispatchResult {
				if is_undeployed && !substate.has_permission_to_call(&source) {
					return Err(Error::<T>::NoPermission.into());
				}
				Ok(())
			},
		)?;

		let pre_storage_usage = Module::<T>::storage_usage(target);
		substate.inc_nonce(source);

		frame_support::storage::with_transaction(|| {
			if let Err(e) = Self::transfer(source, target, value) {
				return TransactionOutcome::Rollback(Err(e));
			}

			if let Err(e) = Self::deduct_storage(source, target, storage_limit) {
				return TransactionOutcome::Rollback(Err(e));
			}

			let code = substate.code(target);
			let (reason, out) =
				substate.execute(source, target, U256::from(value.saturated_into::<u128>()), code, input);

			let mut call_info = CallInfo {
				exit_reason: reason.clone(),
				output: out,
				used_gas: U256::from(substate.used_gas()),
				used_storage: U256::from(substate.used_storage()),
			};

			debug::debug!(
				target: "evm",
				"call-result: call_info {:?}",
				call_info
			);

			if !reason.is_succeed() {
				return TransactionOutcome::Rollback(Ok(call_info));
			}

			let storage_usage = Module::<T>::storage_usage(target);
			if storage_usage != pre_storage_usage {
				if let Some(delta) = storage_usage.checked_sub(pre_storage_usage) {
					if let Err(e) = substate.storagemeter.record_cost(delta) {
						call_info.exit_reason = e.into();
						return TransactionOutcome::Rollback(Ok(call_info));
					}
				} else if let Some(delta) = pre_storage_usage.checked_sub(storage_usage) {
					if let Err(e) = substate.storagemeter.record_refund(delta) {
						call_info.exit_reason = e.into();
						return TransactionOutcome::Rollback(Ok(call_info));
					}
				}
			}
			if let Err(e) = Self::refund_storage(source, target, substate.storagemeter.storage()) {
				debug::debug!(
					target: "evm",
					"call-result: refund_storage {:?}",
					e
				);
				call_info.exit_reason = ExitReason::Error(ExitError::Other("RefundedStorageFailed".into()));
				return TransactionOutcome::Rollback(Ok(call_info));
			}
			call_info.used_storage = U256::from(substate.used_storage());

			TransactionOutcome::Commit(Ok(call_info))
		})
	}

	pub fn create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u32,
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
		gas_limit: u32,
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
		gas_limit: u32,
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
