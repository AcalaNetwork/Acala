//! EVM stack-based runner.

use crate::precompiles::Precompiles;
use crate::runner::Runner as RunnerT;
use crate::{AccountCodes, AccountStorages, AddressMapping, Error, Event, Module, Trait};
use evm::backend::{Apply, ApplyBackend, Backend as BackendT};
use evm::executor::StackExecutor;
use evm::ExitReason;
use frame_support::{
	debug, ensure,
	storage::{StorageDoubleMap, StorageMap},
	traits::{Currency, Get},
};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_evm::{Account, CallInfo, CreateInfo, ExecutionInfo, Log, Vicinity};
use sp_runtime::{traits::UniqueSaturatedInto, SaturatedConversion};
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

#[derive(Default)]
pub struct Runner<T: Trait> {
	_marker: PhantomData<T>,
}

impl<T: Trait> Runner<T> {
	/// Execute an EVM operation.
	pub fn execute<F, R>(source: H160, value: U256, gas_limit: u32, f: F) -> Result<ExecutionInfo<R>, Error<T>>
	where
		F: FnOnce(&mut StackExecutor<Backend<T>>) -> (ExitReason, R),
	{
		let vicinity = Vicinity {
			gas_price: U256::one(),
			origin: source,
		};

		let mut backend = Backend::<T>::new(&vicinity);
		let mut executor =
			StackExecutor::new_with_precompile(&backend, gas_limit as usize, T::config(), T::Precompiles::execute);

		let total_fee = U256::from(gas_limit);
		let total_payment = value.checked_add(total_fee).ok_or(Error::<T>::PaymentOverflow)?;
		let source_account = Module::<T>::account_basic(&source);
		ensure!(source_account.balance >= total_payment, Error::<T>::BalanceLow);
		executor
			.withdraw(source, total_fee)
			.map_err(|_| Error::<T>::WithdrawFailed)?;

		let (reason, retv) = f(&mut executor);

		let used_gas = U256::from(executor.used_gas());
		let actual_fee = executor.fee(U256::one());
		debug::debug!(
			target: "evm",
			"Execution {:?} [source: {:?}, value: {}, gas_limit: {}, actual_fee: {}]",
			reason,
			source,
			value,
			gas_limit,
			actual_fee
		);
		executor.deposit(source, total_fee.saturating_sub(actual_fee));

		let (values, logs) = executor.deconstruct();
		let logs_data = logs.into_iter().collect::<Vec<_>>();
		backend.apply(values, logs_data.clone(), true);

		Ok(ExecutionInfo {
			value: retv,
			exit_reason: reason,
			used_gas,
			logs: logs_data,
		})
	}
}

impl<T: Trait> RunnerT<T> for Runner<T> {
	type Error = Error<T>;

	fn call(source: H160, target: H160, input: Vec<u8>, value: U256, gas_limit: u32) -> Result<CallInfo, Self::Error> {
		Self::execute(source, value, gas_limit, |executor| {
			executor.transact_call(source, target, value, input, gas_limit as usize)
		})
	}

	fn create(source: H160, init: Vec<u8>, value: U256, gas_limit: u32) -> Result<CreateInfo, Self::Error> {
		Self::execute(source, value, gas_limit, |executor| {
			let address = executor.create_address(evm::CreateScheme::Legacy { caller: source });
			(
				executor.transact_create(source, value, init, gas_limit as usize),
				address,
			)
		})
	}

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: U256,
		gas_limit: u32,
	) -> Result<CreateInfo, Self::Error> {
		let code_hash = H256::from_slice(Keccak256::digest(&init).as_slice());
		Self::execute(source, value, gas_limit, |executor| {
			let address = executor.create_address(evm::CreateScheme::Create2 {
				caller: source,
				code_hash,
				salt,
			});
			(
				executor.transact_create2(source, value, init, salt, gas_limit as usize),
				address,
			)
		})
	}
}

/// Substrate backend for EVM.
pub struct Backend<'vicinity, T> {
	vicinity: &'vicinity Vicinity,
	_marker: PhantomData<T>,
}

impl<'vicinity, T: Trait> Backend<'vicinity, T> {
	/// Create a new backend with given vicinity.
	pub fn new(vicinity: &'vicinity Vicinity) -> Self {
		Self {
			vicinity,
			_marker: PhantomData,
		}
	}
	#[allow(clippy::comparison_chain)]
	fn mutate_account_basic(&self, address: &H160, new: Account) {
		let account_id = T::AddressMapping::into_account_id(*address);
		let current = Module::<T>::account_basic(address);

		if current.nonce < new.nonce {
			// ASSUME: in one single EVM transaction, the nonce will not increase more than
			// `u32::max_value()`.
			for _ in 0u32..(new.nonce - current.nonce).unique_saturated_into() {
				frame_system::Module::<T>::inc_account_nonce(&account_id);
			}
		}

		if current.balance > new.balance {
			let diff = current.balance - new.balance;
			T::Currency::slash(&account_id, diff.saturated_into::<u128>().unique_saturated_into());
		} else if current.balance < new.balance {
			let diff = new.balance - current.balance;
			T::Currency::deposit_creating(&account_id, diff.saturated_into::<u128>().unique_saturated_into());
		}
	}
}

impl<'vicinity, T: Trait> BackendT for Backend<'vicinity, T> {
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
			H256::from_slice(frame_system::Module::<T>::block_hash(number).as_ref())
		}
	}

	fn block_number(&self) -> U256 {
		let number: u128 = frame_system::Module::<T>::block_number().unique_saturated_into();
		U256::from(number)
	}

	fn block_coinbase(&self) -> H160 {
		H160::default()
	}

	fn block_timestamp(&self) -> U256 {
		let now: u128 = pallet_timestamp::Module::<T>::get().unique_saturated_into();
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
		let account = Module::<T>::account_basic(&address);

		evm::backend::Basic {
			balance: account.balance,
			nonce: account.nonce,
		}
	}

	fn code_size(&self, address: H160) -> usize {
		AccountCodes::decode_len(&address).unwrap_or(0)
	}

	fn code_hash(&self, address: H160) -> H256 {
		H256::from_slice(Keccak256::digest(&AccountCodes::get(&address)).as_slice())
	}

	fn code(&self, address: H160) -> Vec<u8> {
		AccountCodes::get(&address)
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		AccountStorages::get(address, index)
	}
}

impl<'vicinity, T: Trait> ApplyBackend for Backend<'vicinity, T> {
	fn apply<A, I, L>(&mut self, values: A, logs: L, delete_empty: bool)
	where
		A: IntoIterator<Item = Apply<I>>,
		I: IntoIterator<Item = (H256, H256)>,
		L: IntoIterator<Item = evm::backend::Log>,
	{
		for apply in values {
			match apply {
				Apply::Modify {
					address,
					basic,
					code,
					storage,
					reset_storage,
				} => {
					self.mutate_account_basic(
						&address,
						Account {
							nonce: basic.nonce,
							balance: basic.balance,
						},
					);

					if let Some(code) = code {
						debug::debug!(
							target: "evm",
							"Inserting code ({} bytes) at {:?}",
							code.len(),
							address
						);
						AccountCodes::insert(address, code);
					}

					if reset_storage {
						AccountStorages::remove_prefix(address);
					}

					for (index, value) in storage {
						if value == H256::default() {
							debug::debug!(
								target: "evm",
								"Removing storage for {:?} [index: {:?}]",
								address,
								index
							);
							AccountStorages::remove(address, index);
						} else {
							debug::debug!(
								target: "evm",
								"Updating storage for {:?} [index: {:?}, value: {:?}]",
								address,
								index,
								value
							);
							AccountStorages::insert(address, index, value);
						}
					}

					if delete_empty {
						Module::<T>::remove_account_if_empty(&address);
					}
				}
				Apply::Delete { address } => {
					debug::debug!(
						target: "evm",
						"Deleting account at {:?}",
						address
					);
					Module::<T>::remove_account(&address)
				}
			}
		}

		for log in logs {
			debug::trace!(
				target: "evm",
				"Inserting log for {:?}, topics ({}) {:?}, data ({}): {:?}]",
				log.address,
				log.topics.len(),
				log.topics,
				log.data.len(),
				log.data
			);
			Module::<T>::deposit_event(Event::<T>::Log(Log {
				address: log.address,
				topics: log.topics,
				data: log.data,
			}));
		}
	}
}
