// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use crate::runner::{
	handler::{Handler, StorageMeterHandlerImpl},
	storage_meter::{StorageMeter, StorageMeterHandler},
};
use codec::{Decode, Encode};
use evm::Config as EvmConfig;
use frame_support::{
	dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo},
	ensure,
	error::BadOrigin,
	pallet_prelude::*,
	traits::{
		BalanceStatus, Currency, EnsureOrigin, ExistenceRequirement, Get, MaxEncodedLen, NamedReservableCurrency,
		OnKilledAccount,
	},
	transactional,
	weights::{Pays, PostDispatchInfo, Weight},
	BoundedVec, RuntimeDebug,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*, EnsureOneOf, EnsureRoot, EnsureSigned};
use primitive_types::{H256, U256};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use sp_runtime::{
	traits::{
		Convert, DispatchInfoOf, One, PostDispatchInfoOf, Saturating, SignedExtension, UniqueSaturatedInto, Zero,
	},
	transaction_validity::TransactionValidityError,
	Either, TransactionOutcome,
};
use sp_std::{convert::TryInto, marker::PhantomData, prelude::*};

pub use support::{
	AddressMapping, EVMStateRentTrait, ExecutionMode, InvokeContext, TransactionPayment, EVM as EVMTrait,
};

pub use crate::precompiles::{Precompile, Precompiles};
pub use crate::runner::Runner;
pub use evm::{Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed};
pub use orml_traits::currency::TransferAll;
pub use primitives::{
	evm::{Account, CallInfo, CreateInfo, EvmAddress, Log, Vicinity},
	ReserveIdentifier, MIRRORED_NFT_ADDRESS_START,
};

pub mod precompiles;
pub mod runner;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type NegativeImbalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;
pub const RESERVE_ID_STORAGE_DEPOSIT: ReserveIdentifier = ReserveIdentifier::EvmStorageDeposit;
pub const RESERVE_ID_DEVELOPER_DEPOSIT: ReserveIdentifier = ReserveIdentifier::EvmDeveloperDeposit;

// Initially based on Istanbul hard fork configuration.
static ACALA_CONFIG: EvmConfig = EvmConfig {
	gas_ext_code: 700,
	gas_ext_code_hash: 700,
	gas_balance: 700,
	gas_sload: 800,
	gas_sstore_set: 20000,
	gas_sstore_reset: 5000,
	refund_sstore_clears: 0, // no gas refund
	gas_suicide: 5000,
	gas_suicide_new_account: 25000,
	gas_call: 700,
	gas_expbyte: 50,
	gas_transaction_create: 53000,
	gas_transaction_call: 21000,
	gas_transaction_zero_data: 4,
	gas_transaction_non_zero_data: 16,
	sstore_gas_metering: false,         // no gas refund
	sstore_revert_under_stipend: false, // ignored
	err_on_call_with_more_gas: false,
	empty_considered_exists: false,
	create_increase_nonce: true,
	call_l64_after_gas: true,
	stack_limit: 1024,
	memory_limit: usize::max_value(),
	call_stack_limit: 1024,
	create_contract_limit: None, // ignored
	call_stipend: 2300,
	has_delegate_call: true,
	has_create2: true,
	has_revert: true,
	has_return_data: true,
	has_bitwise_shifting: true,
	has_chain_id: true,
	has_self_balance: true,
	has_ext_code_hash: true,
	estimate: false,
};

#[frame_support::pallet]
pub mod module {
	use crate::runner::handler;

	use super::*;

	/// EVM module trait
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_timestamp::Config {
		/// Mapping from address to account id.
		type AddressMapping: AddressMapping<Self::AccountId>;

		/// Currency type for withdraw and balance storage.
		type Currency: Currency<Self::AccountId>
			+ NamedReservableCurrency<Self::AccountId, ReserveIdentifier = ReserveIdentifier>;

		/// Merge free balance from source to dest.
		type TransferAll: TransferAll<Self::AccountId>;

		/// Charge extra bytes for creating a contract, would be reserved until
		/// the contract deleted.
		#[pallet::constant]
		type NewContractExtraBytes: Get<u32>;

		/// Storage required for per byte.
		#[pallet::constant]
		type StorageDepositPerByte: Get<BalanceOf<Self>>;

		/// Contract max code size.
		#[pallet::constant]
		type MaxCodeSize: Get<u32>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Precompiles associated with this EVM engine.
		type Precompiles: Precompiles;

		/// Chain ID of EVM.
		#[pallet::constant]
		type ChainId: Get<u64>;

		/// Convert gas to weight.
		type GasToWeight: Convert<u64, Weight>;

		/// ChargeTransactionPayment convert weight to fee.
		type ChargeTransactionPayment: TransactionPayment<Self::AccountId, BalanceOf<Self>, NegativeImbalanceOf<Self>>;

		/// EVM config used in the module.
		fn config() -> &'static EvmConfig {
			&ACALA_CONFIG
		}

		/// Required origin for creating system contract.
		type NetworkContractOrigin: EnsureOrigin<Self::Origin>;

		/// The EVM address for creating system contract.
		#[pallet::constant]
		type NetworkContractSource: Get<EvmAddress>;

		/// Deposit for the developer.
		#[pallet::constant]
		type DeveloperDeposit: Get<BalanceOf<Self>>;

		/// The fee for deploying the contract.
		#[pallet::constant]
		type DeploymentFee: Get<BalanceOf<Self>>;

		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		type FreeDeploymentOrigin: EnsureOrigin<Self::Origin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
	pub struct ContractInfo {
		pub code_hash: H256,
		pub maintainer: EvmAddress,
		pub deployed: bool,
	}

	#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
	pub struct AccountInfo<T: Config> {
		pub nonce: T::Index,
		pub contract_info: Option<ContractInfo>,
	}

	impl<T: Config> AccountInfo<T> {
		pub fn new(nonce: T::Index, contract_info: Option<ContractInfo>) -> Self {
			Self { nonce, contract_info }
		}
	}

	#[derive(Clone, Copy, Eq, PartialEq, RuntimeDebug, Encode, Decode, MaxEncodedLen)]
	pub struct CodeInfo {
		pub code_size: u32,
		pub ref_count: u32,
	}

	#[cfg(feature = "std")]
	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, Serialize, Deserialize)]
	/// Account definition used for genesis block construction.
	pub struct GenesisAccount<Balance, Index> {
		/// Account nonce.
		pub nonce: Index,
		/// Account balance.
		pub balance: Balance,
		/// Full account storage.
		pub storage: std::collections::BTreeMap<H256, H256>,
		/// Account code.
		pub code: Vec<u8>,
	}

	/// The EVM accounts info.
	///
	/// Accounts: map EvmAddress => Option<AccountInfo<T>>
	#[pallet::storage]
	#[pallet::getter(fn accounts)]
	pub type Accounts<T: Config> = StorageMap<_, Twox64Concat, EvmAddress, AccountInfo<T>, OptionQuery>;

	/// The storage usage for contracts. Including code size, extra bytes and total AccountStorages
	/// size.
	///
	/// Accounts: map EvmAddress => u32
	#[pallet::storage]
	#[pallet::getter(fn contract_storage_sizes)]
	pub type ContractStorageSizes<T: Config> = StorageMap<_, Twox64Concat, EvmAddress, u32, ValueQuery>;

	/// The storages for EVM contracts.
	///
	/// AccountStorages: double_map EvmAddress, H256 => H256
	#[pallet::storage]
	#[pallet::getter(fn account_storages)]
	pub type AccountStorages<T: Config> =
		StorageDoubleMap<_, Twox64Concat, EvmAddress, Blake2_128Concat, H256, H256, ValueQuery>;

	/// The code for EVM contracts.
	/// Key is Keccak256 hash of code.
	///
	/// Codes: H256 => Vec<u8>
	#[pallet::storage]
	#[pallet::getter(fn codes)]
	pub type Codes<T: Config> = StorageMap<_, Identity, H256, BoundedVec<u8, T::MaxCodeSize>, ValueQuery>;

	/// The code info for EVM contracts.
	/// Key is Keccak256 hash of code.
	///
	/// CodeInfos: H256 => Option<CodeInfo>
	#[pallet::storage]
	#[pallet::getter(fn code_infos)]
	pub type CodeInfos<T: Config> = StorageMap<_, Identity, H256, CodeInfo, OptionQuery>;

	/// Next available system contract address.
	///
	/// NetworkContractIndex: u64
	#[pallet::storage]
	#[pallet::getter(fn network_contract_index)]
	pub type NetworkContractIndex<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Extrinsics origin for the current transaction.
	///
	/// ExtrinsicOrigin: Option<AccountId>
	#[pallet::storage]
	#[pallet::getter(fn extrinsic_origin)]
	pub type ExtrinsicOrigin<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub accounts: std::collections::BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>,
		pub treasury: T::AccountId,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				accounts: Default::default(),
				treasury: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let treasury = T::AddressMapping::get_or_create_evm_address(&self.treasury);
			let mut handler = handler::StorageMeterHandlerImpl::<T>::new(treasury);

			self.accounts.iter().for_each(|(address, account)| {
				let account_id = T::AddressMapping::get_account_id(address);

				let account_info = <AccountInfo<T>>::new(account.nonce, None);
				<Accounts<T>>::insert(address, account_info);

				T::Currency::deposit_creating(&account_id, account.balance);

				if !account.code.is_empty() {
					// if code len > 0 then it's a contract
					let source = T::NetworkContractSource::get();
					let vicinity = Vicinity {
						gas_price: U256::one(),
						origin: source,
					};
					let storage_limit = 0;
					let contract_address = *address;
					let code = account.code.clone();

					let mut storage_meter_handler = StorageMeterHandlerImpl::<T>::new(vicinity.origin);
					let storage_meter = StorageMeter::new(&mut storage_meter_handler, contract_address, storage_limit)
						.expect("Genesis contract failed to new storage_meter");

					let mut substate = Handler::<T>::new(&vicinity, 2_100_000, storage_meter, false, T::config());
					let (reason, out) =
						substate.execute(source, contract_address, Default::default(), code, Vec::new());

					assert!(
						reason.is_succeed(),
						"Genesis contract failed to execute, error: {:?}",
						reason
					);

					<Pallet<T>>::on_contract_initialization(&contract_address, &source, out)
						.expect("Genesis contract failed to initialize");

					#[cfg(not(feature = "with-ethereum-compatibility"))]
					<Pallet<T>>::mark_deployed(*address, None).expect("Genesis contract failed to deploy");

					let mut count = 0;
					for (index, value) in &account.storage {
						AccountStorages::<T>::insert(address, index, value);
						count += 1;
					}

					let storage = count * handler::STORAGE_SIZE;
					handler
						.reserve_storage(storage)
						.expect("Genesis contract failed to reserve storage");
					handler
						.charge_storage(address, storage, 0)
						.expect("Genesis contract failed to charge storage");
				}
			});
			NetworkContractIndex::<T>::put(MIRRORED_NFT_ADDRESS_START);
		}
	}

	/// EVM events
	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Ethereum events from contracts.
		Log(Log),
		/// A contract has been created at given \[address\].
		Created(EvmAddress),
		/// A contract was attempted to be created, but the execution failed.
		/// \[contract, exit_reason, output\]
		CreatedFailed(EvmAddress, ExitReason, Vec<u8>),
		/// A \[contract\] has been executed successfully with states applied.
		Executed(EvmAddress),
		/// A contract has been executed with errors. States are reverted with
		/// only gas fees applied. \[contract, exit_reason, output\]
		ExecutedFailed(EvmAddress, ExitReason, Vec<u8>),
		/// A deposit has been made at a given address. \[sender, address,
		/// value\]
		BalanceDeposit(T::AccountId, EvmAddress, U256),
		/// A withdrawal has been made from a given address. \[sender, address,
		/// value\]
		BalanceWithdraw(T::AccountId, EvmAddress, U256),
		/// A quota has been added at a given address. \[address, bytes\]
		AddStorageQuota(EvmAddress, u32),
		/// A quota has been removed at a given address. \[address, bytes\]
		RemoveStorageQuota(EvmAddress, u32),
		/// Transferred maintainer. \[contract, address\]
		TransferredMaintainer(EvmAddress, EvmAddress),
		/// Canceled the transfer maintainer. \[contract, address\]
		CanceledTransferMaintainer(EvmAddress, EvmAddress),
		/// Confirmed the transfer maintainer. \[contract, address\]
		ConfirmedTransferMaintainer(EvmAddress, EvmAddress),
		/// Rejected the transfer maintainer. \[contract, address\]
		RejectedTransferMaintainer(EvmAddress, EvmAddress),
		/// Enabled contract development. \[who\]
		ContractDevelopmentEnabled(T::AccountId),
		/// Disabled contract development. \[who\]
		ContractDevelopmentDisabled(T::AccountId),
		/// Deployed contract. \[contract\]
		ContractDeployed(EvmAddress),
		/// Set contract code. \[contract\]
		ContractSetCode(EvmAddress),
		/// Selfdestructed contract code. \[contract\]
		ContractSelfdestructed(EvmAddress),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Address not mapped
		AddressNotMapped,
		/// Contract not found
		ContractNotFound,
		/// No permission
		NoPermission,
		/// Number out of bound in calculation.
		NumOutOfBound,
		/// Storage exceeds max code size
		StorageExceedsStorageLimit,
		/// Contract development is not enabled
		ContractDevelopmentNotEnabled,
		/// Contract development is already enabled
		ContractDevelopmentAlreadyEnabled,
		/// Contract already deployed
		ContractAlreadyDeployed,
		/// Contract exceeds max code size
		ContractExceedsMaxCodeSize,
		/// Storage usage exceeds storage limit
		OutOfStorage,
		/// Charge fee failed
		ChargeFeeFailed,
		/// Contract cannot be killed due to reference count
		CannotKillContract,
		/// Contract address conflicts with the system contract
		ConflictContractAddress,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Issue an EVM call operation. This is similar to a message call
		/// transaction in Ethereum.
		///
		/// - `target`: the contract address to call
		/// - `input`: the data supplied for the call
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		pub fn call(
			origin: OriginFor<T>,
			target: EvmAddress,
			input: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = Runner::<T>::call(
				source,
				source,
				target,
				input,
				value,
				gas_limit,
				storage_limit,
				T::config(),
			)?;

			if info.exit_reason.is_succeed() {
				Pallet::<T>::deposit_event(Event::<T>::Executed(target));
			} else {
				Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed(target, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes,
			})
		}

		/// Issue an EVM call operation on a scheduled contract call, and
		/// refund the unused gas reserved when the call was scheduled.
		///
		/// - `from`: the address the scheduled call originates from
		/// - `target`: the contract address to call
		/// - `input`: the data supplied for the call
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		#[transactional]
		pub fn scheduled_call(
			origin: OriginFor<T>,
			from: EvmAddress,
			target: EvmAddress,
			input: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let _from_account = T::AddressMapping::get_account_id(&from);
			let _payed: NegativeImbalanceOf<T>;
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			{
				// unreserve the transaction fee for gas_limit
				let weight = T::GasToWeight::convert(gas_limit);
				let (_, imbalance) = T::ChargeTransactionPayment::unreserve_and_charge_fee(&_from_account, weight)
					.map_err(|_| Error::<T>::ChargeFeeFailed)?;
				_payed = imbalance;
			}

			let info = Runner::<T>::call(from, from, target, input, value, gas_limit, storage_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Pallet::<T>::deposit_event(Event::<T>::Executed(target));
			} else {
				Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed(target, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			#[cfg(not(feature = "with-ethereum-compatibility"))]
			{
				use sp_runtime::traits::Zero;
				let refund_gas = gas_limit.saturating_sub(used_gas);
				if !refund_gas.is_zero() {
					// ignore the result to continue. if it fails, just the user will not
					// be refunded, there will not increase user balance.
					let res = T::ChargeTransactionPayment::refund_fee(
						&_from_account,
						T::GasToWeight::convert(refund_gas),
						_payed,
					);
					debug_assert!(res.is_ok());
				}
			}

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes,
			})
		}

		/// Issue an EVM create operation. This is similar to a contract
		/// creation transaction in Ethereum.
		///
		/// - `init`: the data supplied for the contract's constructor
		/// - `value`: the amount sent to the contract upon creation
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		pub fn create(
			origin: OriginFor<T>,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = Runner::<T>::create(source, init, value, gas_limit, storage_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Pallet::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Pallet::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes,
			})
		}

		/// Issue an EVM create2 operation.
		///
		/// - `target`: the contract address to call
		/// - `init`: the data supplied for the contract's constructor
		/// - `salt`: used for generating the new contract's address
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		pub fn create2(
			origin: OriginFor<T>,
			init: Vec<u8>,
			salt: H256,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = Runner::<T>::create2(source, init, salt, value, gas_limit, storage_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Pallet::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Pallet::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes,
			})
		}

		/// Issue an EVM create operation. The next available system contract
		/// address will be used as created contract address.
		///
		/// - `init`: the data supplied for the contract's constructor
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		pub fn create_network_contract(
			origin: OriginFor<T>,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			let source = T::NetworkContractSource::get();
			let address = EvmAddress::from_low_u64_be(Self::network_contract_index());
			let info =
				Runner::<T>::create_at_address(source, init, value, address, gas_limit, storage_limit, T::config())?;

			NetworkContractIndex::<T>::mutate(|v| *v = v.saturating_add(One::one()));

			if info.exit_reason.is_succeed() {
				Pallet::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Pallet::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes,
			})
		}

		/// Transfers Contract maintainership to a new EVM Address.
		///
		/// - `contract`: the contract whose maintainership is being transferred, the caller must be
		///   the contract's maintainer
		/// - `new_maintainer`: the address of the new maintainer
		#[pallet::weight(<T as Config>::WeightInfo::transfer_maintainer())]
		#[transactional]
		pub fn transfer_maintainer(
			origin: OriginFor<T>,
			contract: EvmAddress,
			new_maintainer: EvmAddress,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_transfer_maintainer(who, contract, new_maintainer)?;

			Pallet::<T>::deposit_event(Event::<T>::TransferredMaintainer(contract, new_maintainer));

			Ok(().into())
		}

		/// Mark a given contract as deployed.
		///
		/// - `contract`: The contract to mark as deployed, the caller must the contract's
		///   maintainer
		#[pallet::weight(<T as Config>::WeightInfo::deploy())]
		#[transactional]
		pub fn deploy(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let address = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
			T::Currency::transfer(
				&who,
				&T::TreasuryAccount::get(),
				T::DeploymentFee::get(),
				ExistenceRequirement::AllowDeath,
			)?;
			Self::mark_deployed(contract, Some(address))?;
			Pallet::<T>::deposit_event(Event::<T>::ContractDeployed(contract));
			Ok(().into())
		}

		/// Mark a given contract as deployed without paying the deployment fee
		///
		/// - `contract`: The contract to mark as deployed, the caller must be the contract's
		///   maintainer.
		#[pallet::weight(<T as Config>::WeightInfo::deploy_free())]
		#[transactional]
		pub fn deploy_free(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResultWithPostInfo {
			T::FreeDeploymentOrigin::ensure_origin(origin)?;
			Self::mark_deployed(contract, None)?;
			Pallet::<T>::deposit_event(Event::<T>::ContractDeployed(contract));
			Ok(().into())
		}

		/// Mark the caller's address to allow contract development.
		/// This allows the address to interact with non-deployed contracts.
		#[pallet::weight(<T as Config>::WeightInfo::enable_contract_development())]
		#[transactional]
		pub fn enable_contract_development(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &who).is_zero(),
				Error::<T>::ContractDevelopmentAlreadyEnabled
			);
			T::Currency::ensure_reserved_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &who, T::DeveloperDeposit::get())?;
			Pallet::<T>::deposit_event(Event::<T>::ContractDevelopmentEnabled(who));
			Ok(().into())
		}

		/// Mark the caller's address to disable contract development.
		/// This disallows the address to interact with non-deployed contracts.
		#[pallet::weight(<T as Config>::WeightInfo::disable_contract_development())]
		#[transactional]
		pub fn disable_contract_development(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				!T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &who).is_zero(),
				Error::<T>::ContractDevelopmentNotEnabled
			);
			T::Currency::unreserve_all_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &who);
			Pallet::<T>::deposit_event(Event::<T>::ContractDevelopmentDisabled(who));
			Ok(().into())
		}

		/// Set the code of a contract at a given address.
		///
		/// - `contract`: The contract whose code is being set, must not be marked as deployed
		/// - `code`: The new ABI bundle for the contract
		#[pallet::weight(<T as Config>::WeightInfo::set_code())]
		#[transactional]
		pub fn set_code(origin: OriginFor<T>, contract: EvmAddress, code: Vec<u8>) -> DispatchResultWithPostInfo {
			let root_or_signed = Self::ensure_root_or_signed(origin)?;
			Self::do_set_code(root_or_signed, contract, code)?;

			Pallet::<T>::deposit_event(Event::<T>::ContractSetCode(contract));

			Ok(().into())
		}

		/// Remove a contract at a given address.
		///
		/// - `contract`: The contract to remove, must not be marked as deployed
		#[pallet::weight(<T as Config>::WeightInfo::selfdestruct())]
		#[transactional]
		pub fn selfdestruct(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let maintainer = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
			Self::do_selfdestruct(who, &maintainer, contract)?;

			Pallet::<T>::deposit_event(Event::<T>::ContractSelfdestructed(contract));

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	#[transactional]
	pub fn remove_contract(address: &EvmAddress, dest: &EvmAddress) -> Result<u32, DispatchError> {
		let address_account = T::AddressMapping::get_account_id(&address);
		let dest_account = T::AddressMapping::get_account_id(&dest);

		let size = Accounts::<T>::try_mutate_exists(address, |account_info| -> Result<u32, DispatchError> {
			let account_info = account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info.contract_info.take().ok_or(Error::<T>::ContractNotFound)?;

			T::TransferAll::transfer_all(&address_account, &dest_account)?;

			CodeInfos::<T>::mutate_exists(&contract_info.code_hash, |maybe_code_info| {
				if let Some(code_info) = maybe_code_info.as_mut() {
					code_info.ref_count = code_info.ref_count.saturating_sub(1);
					if code_info.ref_count == 0 {
						Codes::<T>::remove(&contract_info.code_hash);
						*maybe_code_info = None;
					}
				} else {
					// code info removed while still having reference to it?
					debug_assert!(false);
				}
			});

			AccountStorages::<T>::remove_prefix(address);

			let size = ContractStorageSizes::<T>::take(address);

			Ok(size)
		})?;

		// this should happen after `Accounts` is updated because this could trigger another updates on
		// `Accounts`
		frame_system::Pallet::<T>::dec_providers(&address_account).map_err(|e| match e {
			frame_system::DecRefError::ConsumerRemaining => DispatchError::ConsumerRemaining,
		})?;

		Ok(size)
	}

	/// Removes an account from Accounts and AccountStorages.
	pub fn remove_account(address: &EvmAddress) -> Result<(), ExitError> {
		// Deref code, and remove it if ref count is zero.
		if let Some(AccountInfo {
			contract_info: Some(contract_info),
			..
		}) = Self::accounts(address)
		{
			CodeInfos::<T>::mutate_exists(&contract_info.code_hash, |maybe_code_info| {
				if let Some(code_info) = maybe_code_info.as_mut() {
					code_info.ref_count = code_info.ref_count.saturating_sub(1);
					if code_info.ref_count == 0 {
						Codes::<T>::remove(&contract_info.code_hash);
						*maybe_code_info = None;
					}
				}
			});
		}

		if let Some(AccountInfo {
			contract_info: Some(_), ..
		}) = Accounts::<T>::take(address)
		{
			// remove_account can only be called when account is killed. i.e. providers == 0
			// but contract_info should maintain a provider
			// so this should never happen
			debug_assert!(false);
		}

		Ok(())
	}

	/// Get the account basic in EVM format.
	pub fn account_basic(address: &EvmAddress) -> Account {
		let account_id = T::AddressMapping::get_account_id(address);

		let nonce = Self::accounts(address).map_or(Default::default(), |account_info| account_info.nonce);
		let balance = T::Currency::free_balance(&account_id);

		Account {
			nonce: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(nonce)),
			balance: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(balance)),
		}
	}

	/// Get code hash at given address.
	pub fn code_hash_at_address(address: &EvmAddress) -> H256 {
		if let Some(AccountInfo {
			contract_info: Some(contract_info),
			..
		}) = Self::accounts(address)
		{
			contract_info.code_hash
		} else {
			code_hash(&[])
		}
	}

	/// Get code at given address.
	pub fn code_at_address(address: &EvmAddress) -> BoundedVec<u8, T::MaxCodeSize> {
		Self::codes(&Self::code_hash_at_address(address))
	}

	pub fn update_contract_storage_size(address: &EvmAddress, change: i32) {
		if change == 0 {
			return;
		}
		ContractStorageSizes::<T>::mutate(address, |val| {
			if change > 0 {
				*val = val.saturating_add(change as u32);
			} else {
				*val = val.saturating_sub((-change) as u32);
			}
		});
	}

	/// Handler on new contract initialization.
	///
	/// - Create new account for the contract.
	/// - Update codes info.
	/// - Save `code` if not saved yet.
	pub fn on_contract_initialization(
		address: &EvmAddress,
		maintainer: &EvmAddress,
		code: Vec<u8>,
	) -> Result<(), ExitError> {
		let bounded_code: BoundedVec<u8, T::MaxCodeSize> = code.try_into().map_err(|_| ExitError::OutOfGas)?;
		let code_hash = code_hash(&bounded_code.as_slice());
		let code_size = bounded_code.len() as u32;

		let contract_info = ContractInfo {
			code_hash,
			maintainer: *maintainer,
			#[cfg(feature = "with-ethereum-compatibility")]
			deployed: true,
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			deployed: false,
		};

		Self::update_contract_storage_size(
			address,
			code_size.saturating_add(T::NewContractExtraBytes::get()) as i32,
		);

		CodeInfos::<T>::mutate_exists(&code_hash, |maybe_code_info| {
			if let Some(code_info) = maybe_code_info.as_mut() {
				code_info.ref_count = code_info.ref_count.saturating_add(1);
			} else {
				let new = CodeInfo {
					code_size,
					ref_count: 1,
				};
				*maybe_code_info = Some(new);

				Codes::<T>::insert(&code_hash, bounded_code);
			}
		});

		Accounts::<T>::mutate(address, |maybe_account_info| {
			if let Some(account_info) = maybe_account_info.as_mut() {
				account_info.contract_info = Some(contract_info.clone());
			} else {
				let account_info = AccountInfo::<T>::new(Default::default(), Some(contract_info.clone()));
				*maybe_account_info = Some(account_info);
			}
		});

		frame_system::Pallet::<T>::inc_providers(&T::AddressMapping::get_account_id(address));

		Ok(())
	}

	/// Sets a given contract's contract info to a new maintainer.
	fn do_transfer_maintainer(who: T::AccountId, contract: EvmAddress, new_maintainer: EvmAddress) -> DispatchResult {
		Accounts::<T>::get(contract).map_or(Err(Error::<T>::ContractNotFound), |account_info| {
			account_info
				.contract_info
				.map_or(Err(Error::<T>::ContractNotFound), |_| Ok(()))
		})?;

		Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
			let account_info = maybe_account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info
				.contract_info
				.as_mut()
				.ok_or(Error::<T>::ContractNotFound)?;

			let maintainer = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
			ensure!(contract_info.maintainer == maintainer, Error::<T>::NoPermission);

			contract_info.maintainer = new_maintainer;
			Ok(())
		})?;

		Ok(())
	}

	/// Mark contract as deployed
	///
	/// If maintainer is provider then it will check maintainer
	fn mark_deployed(contract: EvmAddress, maintainer: Option<EvmAddress>) -> DispatchResult {
		Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
			if let Some(AccountInfo {
				contract_info: Some(contract_info),
				..
			}) = maybe_account_info.as_mut()
			{
				if let Some(maintainer) = maintainer {
					ensure!(contract_info.maintainer == maintainer, Error::<T>::NoPermission);
				}
				ensure!(!contract_info.deployed, Error::<T>::ContractAlreadyDeployed);
				contract_info.deployed = true;
				Ok(())
			} else {
				Err(Error::<T>::ContractNotFound.into())
			}
		})
	}

	/// Set the code of a contract at a given address.
	///
	/// - Ensures signer is maintainer or root.
	/// - Update codes info.
	/// - Save `code`if not saved yet.
	fn do_set_code(root_or_signed: Either<(), T::AccountId>, contract: EvmAddress, code: Vec<u8>) -> DispatchResult {
		Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
			let account_info = maybe_account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info
				.contract_info
				.as_ref()
				.ok_or(Error::<T>::ContractNotFound)?;

			let source = if let Either::Right(signer) = root_or_signed {
				let maintainer = T::AddressMapping::get_evm_address(&signer).ok_or(Error::<T>::AddressNotMapped)?;
				ensure!(contract_info.maintainer == maintainer, Error::<T>::NoPermission);
				ensure!(!contract_info.deployed, Error::<T>::ContractAlreadyDeployed);
				maintainer
			} else {
				T::NetworkContractSource::get()
			};

			let code_size = code.len() as u32;
			let code_hash = code_hash(&code.as_slice());
			if code_hash == contract_info.code_hash {
				return Ok(());
			}

			ensure!(
				code_size <= T::MaxCodeSize::get(),
				Error::<T>::ContractExceedsMaxCodeSize
			);

			Runner::<T>::create_at_address(
				source,
				code,
				Default::default(),
				contract,
				2_100_000,
				100_000,
				T::config(),
			)
			.map(|_| ())
		})
	}

	/// Selfdestruct a contract at a given address.
	fn do_selfdestruct(who: T::AccountId, maintainer: &EvmAddress, contract: EvmAddress) -> DispatchResult {
		let account_info = Self::accounts(contract).ok_or(Error::<T>::ContractNotFound)?;
		let contract_info = account_info
			.contract_info
			.as_ref()
			.ok_or(Error::<T>::ContractNotFound)?;

		ensure!(contract_info.maintainer == *maintainer, Error::<T>::NoPermission);
		ensure!(!contract_info.deployed, Error::<T>::ContractAlreadyDeployed);

		let storage = Self::remove_contract(&contract, &maintainer)?;

		let contract_account = T::AddressMapping::get_account_id(&contract);

		let amount = T::StorageDepositPerByte::get().saturating_mul(storage.into());
		let val = T::Currency::repatriate_reserved_named(
			&RESERVE_ID_STORAGE_DEPOSIT,
			&contract_account,
			&who,
			amount,
			BalanceStatus::Free,
		)?;
		debug_assert!(val.is_zero());

		Ok(())
	}

	fn ensure_root_or_signed(o: T::Origin) -> Result<Either<(), T::AccountId>, BadOrigin> {
		EnsureOneOf::<T::AccountId, EnsureRoot<T::AccountId>, EnsureSigned<T::AccountId>>::try_origin(o)
			.map_or(Err(BadOrigin), Ok)
	}
}

impl<T: Config> EVMTrait<T::AccountId> for Pallet<T> {
	type Balance = BalanceOf<T>;
	fn execute(
		context: InvokeContext,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		mode: ExecutionMode,
	) -> Result<CallInfo, sp_runtime::DispatchError> {
		let mut config = T::config().clone();
		if let ExecutionMode::EstimateGas = mode {
			config.estimate = true;
		}

		frame_support::storage::with_transaction(|| {
			let result = Runner::<T>::call(
				context.sender,
				context.origin,
				context.contract,
				input,
				value,
				gas_limit,
				storage_limit,
				&config,
			);

			match result {
				Ok(info) => match mode {
					ExecutionMode::Execute => {
						if info.exit_reason.is_succeed() {
							Pallet::<T>::deposit_event(Event::<T>::Executed(context.contract));
							TransactionOutcome::Commit(Ok(info))
						} else {
							Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed(
								context.contract,
								info.exit_reason.clone(),
								info.output.clone(),
							));
							TransactionOutcome::Rollback(Ok(info))
						}
					}
					ExecutionMode::View | ExecutionMode::EstimateGas => TransactionOutcome::Rollback(Ok(info)),
				},
				Err(e) => TransactionOutcome::Rollback(Err(e)),
			}
		})
	}

	/// Get the real origin account and charge storage rent from the origin.
	fn get_origin() -> Option<T::AccountId> {
		ExtrinsicOrigin::<T>::get()
	}

	/// Provide a method to set origin for `on_initialize`
	fn set_origin(origin: T::AccountId) {
		ExtrinsicOrigin::<T>::set(Some(origin));
	}
}

impl<T: Config> EVMStateRentTrait<T::AccountId, BalanceOf<T>> for Pallet<T> {
	fn query_new_contract_extra_bytes() -> u32 {
		T::NewContractExtraBytes::get()
	}

	fn query_storage_deposit_per_byte() -> BalanceOf<T> {
		T::StorageDepositPerByte::get()
	}

	fn query_maintainer(contract: EvmAddress) -> Result<EvmAddress, DispatchError> {
		Accounts::<T>::get(contract).map_or(Err(Error::<T>::ContractNotFound.into()), |account_info| {
			account_info
				.contract_info
				.map_or(Err(Error::<T>::ContractNotFound.into()), |v| Ok(v.maintainer))
		})
	}

	fn query_developer_deposit() -> BalanceOf<T> {
		T::DeveloperDeposit::get()
	}

	fn query_deployment_fee() -> BalanceOf<T> {
		T::DeploymentFee::get()
	}

	fn transfer_maintainer(from: T::AccountId, contract: EvmAddress, new_maintainer: EvmAddress) -> DispatchResult {
		Pallet::<T>::do_transfer_maintainer(from, contract, new_maintainer)
	}
}

pub struct CallKillAccount<T>(PhantomData<T>);
impl<T: Config> OnKilledAccount<T::AccountId> for CallKillAccount<T> {
	fn on_killed_account(who: &T::AccountId) {
		if let Some(address) = T::AddressMapping::get_evm_address(who) {
			let res = Pallet::<T>::remove_account(&address);
			debug_assert!(res.is_ok());
		}
		let address = T::AddressMapping::get_default_evm_address(who);
		let res = Pallet::<T>::remove_account(&address);
		debug_assert!(res.is_ok());
	}
}

pub fn code_hash(code: &[u8]) -> H256 {
	H256::from_slice(Keccak256::digest(code).as_slice())
}

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct SetEvmOrigin<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for SetEvmOrigin<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "SetEvmOrigin")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> SetEvmOrigin<T> {
	pub fn new() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}

impl<T: Config + Send + Sync> Default for SetEvmOrigin<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Config + Send + Sync> SignedExtension for SetEvmOrigin<T> {
	const IDENTIFIER: &'static str = "SetEvmOrigin";
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type Pre = ();

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> Result<(), TransactionValidityError> {
		ExtrinsicOrigin::<T>::set(Some(who.clone()));
		Ok(())
	}

	fn post_dispatch(
		_pre: Self::Pre,
		_info: &DispatchInfoOf<Self::Call>,
		_post_info: &PostDispatchInfoOf<Self::Call>,
		_len: usize,
		_result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		ExtrinsicOrigin::<T>::kill();
		Ok(())
	}
}
