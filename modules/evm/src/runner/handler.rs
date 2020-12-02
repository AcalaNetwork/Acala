#![allow(clippy::type_complexity)]

use crate::{
	AccountInfo, AccountStorages, Accounts, AddressMapping, BalanceOf, Codes, Event, Log, MergeAccount, Module, Trait,
	Vicinity,
};
use evm::{
	Capture, Context, CreateScheme, ExitError, ExitReason, ExitSucceed, ExternalOpcode, Opcode, Runtime, Stack,
	Transfer,
};
use evm_gasometer::{self as gasometer, Gasometer};
use evm_runtime::{Config, Handler as HandlerT};
use frame_support::{
	debug,
	storage::{StorageDoubleMap, StorageMap},
	traits::{Currency, ExistenceRequirement, Get, ReservableCurrency},
};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::{
	traits::{One, UniqueSaturatedInto, Zero},
	SaturatedConversion, TransactionOutcome,
};
use sp_std::{cmp::min, convert::Infallible, marker::PhantomData, rc::Rc, vec::Vec};

pub struct Handler<'vicinity, 'config, T: Trait> {
	pub vicinity: &'vicinity Vicinity,
	pub config: &'config Config,
	pub gasometer: Gasometer<'config>,
	pub precompile:
		fn(H160, &[u8], Option<usize>, &Context) -> Option<Result<(ExitSucceed, Vec<u8>, usize), ExitError>>,
	pub is_static: bool,
	pub _marker: PhantomData<T>,
}

fn l64(gas: usize) -> usize {
	gas - gas / 64
}

impl<'vicinity, 'config, T: Trait> Handler<'vicinity, 'config, T> {
	/// Create a new handler with given vicinity.
	pub fn new_with_precompile(
		vicinity: &'vicinity Vicinity,
		gas_limit: usize,
		is_static: bool,
		config: &'config Config,
		precompile: fn(
			H160,
			&[u8],
			Option<usize>,
			&Context,
		) -> Option<Result<(ExitSucceed, Vec<u8>, usize), ExitError>>,
	) -> Self {
		Self {
			vicinity,
			config,
			is_static,
			gasometer: Gasometer::new(gas_limit, config),
			precompile,
			_marker: PhantomData,
		}
	}

	/// Get used gas for the current executor, given the price.
	pub fn used_gas(&self) -> usize {
		self.gasometer.total_used_gas()
			- min(
				self.gasometer.total_used_gas() / 2,
				self.gasometer.refunded_gas() as usize,
			)
	}

	pub fn execute(
		&mut self,
		caller: H160,
		address: H160,
		value: U256,
		code: Vec<u8>,
		input: Vec<u8>,
	) -> (ExitReason, Vec<u8>) {
		let context = Context {
			caller,
			address,
			apparent_value: value,
		};

		let mut runtime = Runtime::new(Rc::new(code), Rc::new(input), context, self.config);

		let reason = match runtime.run(self) {
			Capture::Exit(s) => s,
			Capture::Trap(_) => unreachable!("Trap is Infallible"),
		};

		match reason {
			ExitReason::Succeed(s) => (s.into(), runtime.machine().return_value()),
			ExitReason::Error(e) => (e.into(), Vec::new()),
			ExitReason::Revert(e) => (e.into(), runtime.machine().return_value()),
			ExitReason::Fatal(e) => {
				self.gasometer.fail();
				(e.into(), Vec::new())
			}
		}
	}

	fn transfer(&self, transfer: Transfer) -> Result<(), ExitError> {
		let source = T::AddressMapping::to_account(&transfer.source);
		let target = T::AddressMapping::to_account(&transfer.target);

		T::Currency::transfer(
			&source,
			&target,
			transfer.value.saturated_into::<u128>().unique_saturated_into(),
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| ExitError::OutOfGas)
	}

	fn reserve(&self, address: H160, value: BalanceOf<T>) -> Result<(), ExitError> {
		let account_id = T::AddressMapping::to_account(&address);

		T::Currency::reserve(&account_id, value).map_err(|_| ExitError::Other("Reserve failed".into()))
	}

	fn unreserve(&self, address: H160, value: BalanceOf<T>) -> Result<(), ExitError> {
		let account_id = T::AddressMapping::to_account(&address);

		if T::Currency::unreserve(&account_id, value).is_zero() {
			Ok(())
		} else {
			Err(ExitError::Other("Unreserve failed".into()))
		}
	}

	pub fn nonce(&self, address: H160) -> U256 {
		let account = Module::<T>::account_basic(&address);
		account.nonce
	}

	pub fn inc_nonce(&self, address: H160) {
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

	pub fn create_address(&self, scheme: CreateScheme) -> H160 {
		match scheme {
			CreateScheme::Create2 {
				caller,
				code_hash,
				salt,
			} => {
				let mut hasher = Keccak256::new();
				hasher.input(&[0xff]);
				hasher.input(&caller[..]);
				hasher.input(&salt[..]);
				hasher.input(&code_hash[..]);
				H256::from_slice(hasher.result().as_slice()).into()
			}
			CreateScheme::Legacy { caller } => {
				let nonce = self.nonce(caller);
				let mut stream = rlp::RlpStream::new_list(2);
				stream.append(&caller);
				stream.append(&nonce);
				H256::from_slice(Keccak256::digest(&stream.out()).as_slice()).into()
			}
			CreateScheme::Fixed(naddress) => naddress,
		}
	}
}

/// Create `try_or_fail` and `try_or_rollback`.
macro_rules! create_try {
	( $map_err:expr ) => {
		macro_rules! try_or_fail {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return Capture::Exit($map_err(e)),
					}
			};
		}

		macro_rules! try_or_rollback {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return TransactionOutcome::Rollback(Capture::Exit($map_err(e))),
					}
			};
		}
	};
}

impl<'vicinity, 'config, T: Trait> HandlerT for Handler<'vicinity, 'config, T> {
	type CreateInterrupt = Infallible;
	type CreateFeedback = Infallible;
	type CallInterrupt = Infallible;
	type CallFeedback = Infallible;

	fn balance(&self, address: H160) -> U256 {
		let account = Module::<T>::account_basic(&address);
		account.balance
	}

	fn code_size(&self, address: H160) -> U256 {
		let code_hash = self.code_hash(address);
		U256::from(Codes::decode_len(&code_hash).unwrap_or(0))
	}

	fn code_hash(&self, address: H160) -> H256 {
		Module::<T>::code_hash_at_address(&address)
	}

	fn code(&self, address: H160) -> Vec<u8> {
		Module::<T>::code_at_address(&address)
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		AccountStorages::get(address, index)
	}

	fn original_storage(&self, _address: H160, _index: H256) -> H256 {
		// We do not have the concept of original storage in the native runner, so we
		// always return empty value. This only affects gas calculation in the current
		// EVM specification.
		H256::default()
	}

	fn gas_left(&self) -> U256 {
		U256::from(self.gasometer.gas())
	}

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

	fn deleted(&self, _address: H160) -> bool {
		// This only affects gas calculation in the current EVM specification.
		// return true to disable suicide gas refund
		true
	}

	fn set_storage(&mut self, address: H160, index: H256, value: H256) -> Result<(), ExitError> {
		if self.is_static {
			return Err(ExitError::OutOfGas);
		}

		<Module<T>>::set_storage(address, index, value);

		Ok(())
	}

	fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError> {
		Module::<T>::deposit_event(Event::<T>::Log(Log { address, topics, data }));

		Ok(())
	}

	fn mark_delete(&mut self, address: H160, target: H160) -> Result<(), ExitError> {
		if self.is_static {
			return Err(ExitError::OutOfGas);
		}

		let source = T::AddressMapping::to_account(&address);
		let dest = T::AddressMapping::to_account(&target);

		// unreserve ContractExistentialDeposit
		self.unreserve(address, T::ContractExistentialDeposit::get())?;

		T::MergeAccount::merge_account(&source, &dest).map_err(|_| ExitError::Other("Remove account failed".into()))?;
		Module::<T>::remove_account(&address);

		Ok(())
	}

	fn create(
		&mut self,
		caller: H160,
		scheme: CreateScheme,
		value: U256,
		init_code: Vec<u8>,
		target_gas: Option<usize>,
	) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Self::CreateInterrupt> {
		debug::debug!(
			target: "evm",
			"handler: create: caller {:?}",
			caller,
		);

		create_try!(|e: ExitError| (e.into(), None, Vec::new()));

		if self.is_static {
			return Capture::Exit((ExitError::OutOfGas.into(), None, Vec::new()));
		}

		let mut after_gas = self.gasometer.gas();
		if self.config.call_l64_after_gas {
			after_gas = l64(after_gas);
		}
		let mut target_gas = target_gas.unwrap_or(after_gas);
		target_gas = min(target_gas, after_gas);
		try_or_fail!(self.gasometer.record_cost(target_gas));

		let mut substate =
			Self::new_with_precompile(self.vicinity, target_gas, self.is_static, self.config, self.precompile);

		let address = self.create_address(scheme);
		substate.inc_nonce(caller);

		frame_support::storage::with_transaction(|| {
			try_or_rollback!(self.transfer(Transfer {
				source: caller,
				target: address,
				value,
			}));

			let contract_existential_deposit = Transfer {
				source: if self.vicinity.creating {
					self.vicinity.origin
				} else {
					caller
				},
				target: address,
				value: U256::from(T::ContractExistentialDeposit::get().saturated_into::<u128>()),
			};

			try_or_rollback!(self.transfer(contract_existential_deposit));
			try_or_rollback!(self.reserve(address, T::ContractExistentialDeposit::get()));

			let (reason, out) = substate.execute(caller, address, value, init_code, Vec::new());

			match reason {
				ExitReason::Succeed(s) => match self.gasometer.record_deposit(out.len()) {
					Ok(()) => {
						try_or_rollback!(self.gasometer.record_stipend(substate.gasometer.gas()));
						try_or_rollback!(self.gasometer.record_refund(substate.gasometer.refunded_gas()));
						substate.inc_nonce(address);
						<Module<T>>::on_contract_initialization(&address, out, None);

						TransactionOutcome::Commit(Capture::Exit((s.into(), Some(address), Vec::new())))
					}
					Err(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), None, Vec::new()))),
				},
				ExitReason::Revert(r) => TransactionOutcome::Rollback(Capture::Exit((r.into(), None, out))),
				ExitReason::Error(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), None, Vec::new()))),
				ExitReason::Fatal(e) => {
					self.gasometer.fail();
					TransactionOutcome::Rollback(Capture::Exit((e.into(), None, Vec::new())))
				}
			}
		})
	}

	fn call(
		&mut self,
		code_address: H160,
		transfer: Option<Transfer>,
		input: Vec<u8>,
		target_gas: Option<usize>,
		is_static: bool,
		context: Context,
	) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
		debug::debug!(
			target: "evm",
			"handler: call: code_address {:?}",
			code_address,
		);

		create_try!(|e: ExitError| (e.into(), Vec::new()));

		if self.is_static && transfer.is_some() {
			return Capture::Exit((ExitError::OutOfGas.into(), Vec::new()));
		}

		let mut after_gas = self.gasometer.gas();
		if self.config.call_l64_after_gas {
			after_gas = l64(after_gas);
		}
		let mut target_gas = target_gas.unwrap_or(after_gas);
		target_gas = min(target_gas, after_gas);

		if let Some(transfer) = transfer.as_ref() {
			if !transfer.value.is_zero() {
				target_gas = target_gas.saturating_add(self.config.call_stipend);
			}
		}

		try_or_fail!(self.gasometer.record_cost(target_gas));

		let code = self.code(code_address);

		frame_support::storage::with_transaction(|| {
			let mut substate = Self::new_with_precompile(
				self.vicinity,
				target_gas,
				self.is_static || is_static,
				self.config,
				self.precompile,
			);

			if let Some(transfer) = transfer {
				try_or_rollback!(self.transfer(transfer));
			}

			if let Some(ret) = (substate.precompile)(code_address, &input, Some(target_gas), &context) {
				return match ret {
					Ok((s, out, cost)) => {
						try_or_rollback!(self.gasometer.record_cost(cost));
						TransactionOutcome::Commit(Capture::Exit((s.into(), out)))
					}
					Err(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), Vec::new()))),
				};
			}

			let (reason, out) = substate.execute(context.caller, context.address, context.apparent_value, code, input);

			match reason {
				ExitReason::Succeed(s) => {
					try_or_rollback!(self.gasometer.record_stipend(substate.gasometer.gas()));
					try_or_rollback!(self.gasometer.record_refund(substate.gasometer.refunded_gas()));

					TransactionOutcome::Commit(Capture::Exit((s.into(), out)))
				}
				ExitReason::Revert(r) => TransactionOutcome::Rollback(Capture::Exit((r.into(), out))),
				ExitReason::Error(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), Vec::new()))),
				ExitReason::Fatal(e) => {
					self.gasometer.fail();
					TransactionOutcome::Rollback(Capture::Exit((e.into(), Vec::new())))
				}
			}
		})
	}

	fn pre_validate(
		&mut self,
		context: &Context,
		opcode: Result<Opcode, ExternalOpcode>,
		stack: &Stack,
	) -> Result<(), ExitError> {
		let (gas_cost, memory_cost) =
			gasometer::opcode_cost(context.address, opcode, stack, self.is_static, &self.config, self)?;

		self.gasometer.record_opcode(gas_cost, memory_cost)?;

		Ok(())
	}
}
