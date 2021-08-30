//! EVM stack-based runner.

use crate::precompiles::PrecompileSet;
use crate::runner::Runner as RunnerT;
use crate::{AccountInfo, Accounts, StorageMeter, StorageMeterHandler};
use crate::{
	AccountStorages, CallInfo, Config, CreateInfo, Error, Event, ExecutionInfo, Pallet, RESERVE_ID_STORAGE_DEPOSIT,
};
use evm::backend::Backend as BackendT;
use evm::executor::{StackExecutor, StackState as StackStateT, StackSubstateMetadata};
use evm::{ExitError, ExitReason, Transfer};
use frame_support::{
	ensure, log,
	traits::{BalanceStatus, Currency, ExistenceRequirement, Get, NamedReservableCurrency},
};
use module_support::AddressMapping;
pub use primitives::{
	evm::{Account, EvmAddress, Log, Vicinity},
	ReserveIdentifier, MIRRORED_NFT_ADDRESS_START,
};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::{
	traits::{One, Saturating, UniqueSaturatedInto, Zero},
	DispatchError, DispatchResult, SaturatedConversion, TransactionOutcome,
};
use sp_std::{boxed::Box, collections::btree_set::BTreeSet, marker::PhantomData, mem, vec::Vec};

pub struct StorageMeterHandlerImpl<T: Config> {
	origin: H160,
	_marker: PhantomData<T>,
}

impl<T: Config> StorageMeterHandlerImpl<T> {
	pub fn new(origin: H160) -> Self {
		Self {
			origin,
			_marker: Default::default(),
		}
	}
}

impl<T: Config> StorageMeterHandler for StorageMeterHandlerImpl<T> {
	fn reserve_storage(&mut self, limit: u32) -> DispatchResult {
		if limit.is_zero() {
			return Ok(());
		}

		log::debug!(
			target: "evm",
			"reserve_storage: from {:?} limit {:?}",
			self.origin, limit,
		);

		let user = T::AddressMapping::get_account_id(&self.origin);

		let amount = T::StorageDepositPerByte::get().saturating_mul(limit.into());

		T::Currency::reserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount)
	}

	fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> DispatchResult {
		let total = limit.saturating_add(refunded);
		let unused = total.saturating_sub(used);
		if unused.is_zero() {
			return Ok(());
		}

		log::debug!(
			target: "evm",
			"unreserve_storage: from {:?} used {:?} refunded {:?} unused {:?}",
			self.origin, used, refunded, unused
		);

		let user = T::AddressMapping::get_account_id(&self.origin);
		let amount = T::StorageDepositPerByte::get().saturating_mul(unused.into());

		// should always be able to unreserve the amount
		// but otherwise we will just ignore the issue here.
		let err_amount = T::Currency::unreserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount);
		debug_assert!(err_amount.is_zero());
		Ok(())
	}

	fn charge_storage(&mut self, contract: &H160, used: u32, refunded: u32) -> DispatchResult {
		if used == refunded {
			return Ok(());
		}

		log::debug!(
			target: "evm",
			"charge_storage: from {:?} contract {:?} used {:?} refunded {:?}",
			&self.origin, contract, used, refunded
		);

		let user = T::AddressMapping::get_account_id(&self.origin);
		let contract_acc = T::AddressMapping::get_account_id(contract);

		if used > refunded {
			let storage = used - refunded;
			let amount = T::StorageDepositPerByte::get().saturating_mul(storage.into());

			// `repatriate_reserved` requires beneficiary is an existing account but
			// contract_acc could be a new account so we need to do
			// unreserve/transfer/reserve.
			// should always be able to unreserve the amount
			// but otherwise we will just ignore the issue here.
			let err_amount = T::Currency::unreserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount);
			debug_assert!(err_amount.is_zero());
			T::Currency::transfer(&user, &contract_acc, amount, ExistenceRequirement::AllowDeath)?;
			T::Currency::reserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &contract_acc, amount)?;
		} else {
			let storage = refunded - used;
			let amount = T::StorageDepositPerByte::get().saturating_mul(storage.into());

			// user can't be a dead account
			let val = T::Currency::repatriate_reserved_named(
				&RESERVE_ID_STORAGE_DEPOSIT,
				&contract_acc,
				&user,
				amount,
				BalanceStatus::Reserved,
			)?;
			debug_assert!(val.is_zero());
		};

		Ok(())
	}

	fn out_of_storage_error(&self) -> DispatchError {
		Error::<T>::OutOfStorage.into()
	}
}
