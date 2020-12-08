//! Native EVM runner.
#![allow(clippy::type_complexity)]

use crate::runner::handler::Handler;
use crate::{
	precompiles::Precompiles, AddressMapping, BalanceOf, CallInfo, Config, CreateInfo, Module, Runner as RunnerT,
	Vicinity,
};
use evm::CreateScheme;
use evm_runtime::Handler as HandlerT;
use frame_support::{
	debug,
	traits::{Currency, ExistenceRequirement, Get, ReservableCurrency},
};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::{DispatchError, SaturatedConversion, TransactionOutcome};
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
		assigned_address: Option<H160>,
		salt: Option<H256>,
		tag: &'static str,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		debug::debug!(
			target: "evm",
			"{:?}: source {:?}, gas_limit: {:?}",
			tag,
			source,
			gas_limit,
		);

		let vicinity = Vicinity {
			gas_price: U256::one(),
			origin: source,
			creating: true,
		};

		let mut substate =
			Handler::<T>::new_with_precompile(&vicinity, gas_limit as usize, false, config, T::Precompiles::execute);

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

			if let Err(e) = Self::transfer_and_reserve_deposit(source, address) {
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

			<Module<T>>::on_contract_initialization(&address, out, None);
			TransactionOutcome::Commit(Ok(create_info))
		})
	}

	fn transfer(source: H160, target: H160, value: BalanceOf<T>) -> Result<(), DispatchError> {
		let from = T::AddressMapping::to_account(&source);
		let to = T::AddressMapping::to_account(&target);
		T::Currency::transfer(&from, &to, value, ExistenceRequirement::AllowDeath)
	}

	fn transfer_and_reserve_deposit(source: H160, target: H160) -> Result<(), DispatchError> {
		let from = T::AddressMapping::to_account(&source);
		let to = T::AddressMapping::to_account(&target);
		T::Currency::transfer(
			&from,
			&to,
			T::ContractExistentialDeposit::get(),
			ExistenceRequirement::AllowDeath,
		)?;
		T::Currency::reserve(&to, T::ContractExistentialDeposit::get())
	}
}

impl<T: Config> RunnerT<T> for Runner<T> {
	fn call(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u32,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError> {
		debug::debug!(
			target: "evm",
			"call: source {:?}, target: {:?}, gas_limit: {:?}",
			source,
			target,
			gas_limit,
		);

		let vicinity = Vicinity {
			gas_price: U256::one(),
			origin: source,
			creating: false,
		};

		let mut substate =
			Handler::<T>::new_with_precompile(&vicinity, gas_limit as usize, false, config, T::Precompiles::execute);

		substate.inc_nonce(source);

		frame_support::storage::with_transaction(|| {
			if let Err(e) = Self::transfer(source, target, value) {
				return TransactionOutcome::Rollback(Err(e));
			}

			let code = substate.code(target);
			let (reason, out) =
				substate.execute(source, target, U256::from(value.saturated_into::<u128>()), code, input);

			let call_info = CallInfo {
				exit_reason: reason.clone(),
				output: out,
				used_gas: U256::from(substate.used_gas()),
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
		})
	}

	fn create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		Self::inner_create(source, init, value, gas_limit, None, None, "create", config)
	}

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: BalanceOf<T>,
		gas_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		Self::inner_create(source, init, value, gas_limit, None, Some(salt), "create2", config)
	}

	fn create_at_address(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		assigned_address: H160,
		gas_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError> {
		Self::inner_create(
			source,
			init,
			value,
			gas_limit,
			Some(assigned_address),
			None,
			"create-system-contract",
			config,
		)
	}
}
