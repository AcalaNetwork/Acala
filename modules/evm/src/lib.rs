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

pub use crate::{
	precompiles::{Precompile, PrecompileSet},
	runner::{
		stack::SubstrateStackState,
		state::{StackExecutor, StackSubstateMetadata},
		storage_meter::StorageMeter,
		Runner,
	},
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo},
	ensure,
	error::BadOrigin,
	log,
	pallet_prelude::*,
	parameter_types,
	traits::{
		BalanceStatus, Currency, EnsureOrigin, ExistenceRequirement, FindAuthor, Get, NamedReservableCurrency,
		OnKilledAccount,
	},
	transactional,
	weights::{Pays, PostDispatchInfo, Weight},
	BoundedVec, RuntimeDebug,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*, EnsureOneOf, EnsureRoot, EnsureSigned};
use hex_literal::hex;
pub use module_evm_utiltity::{
	ethereum::{Log, TransactionAction},
	evm::{self, Config as EvmConfig, Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed},
	Account,
};
pub use module_support::{
	AddressMapping, EVMStateRentTrait, ExecutionMode, InvokeContext, TransactionPayment, EVM as EVMTrait,
};
pub use orml_traits::currency::TransferAll;
use primitive_types::{H160, H256, U256};
pub use primitives::{
	evm::{CallInfo, CreateInfo, EvmAddress, ExecutionInfo, Vicinity},
	ReserveIdentifier, H160_PREFIX_DEXSHARE, H160_PREFIX_TOKEN, MIRRORED_NFT_ADDRESS_START, PRECOMPILE_ADDRESS_START,
	SYSTEM_CONTRACT_ADDRESS_PREFIX,
};
use scale_info::TypeInfo;
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
use sp_std::{collections::btree_map::BTreeMap, convert::TryInto, fmt::Write, marker::PhantomData, prelude::*};

pub mod precompiles;
pub mod runner;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

/// Storage key size and storage value size.
pub const STORAGE_SIZE: u32 = 64;

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
	gas_sload_cold: 0,
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
	gas_access_list_address: 0,
	gas_access_list_storage_key: 0,
	gas_account_access_cold: 0,
	gas_storage_read_warm: 0,
	sstore_gas_metering: false,         // no gas refund
	sstore_revert_under_stipend: false, // ignored
	increase_state_access_gas: false,
	err_on_call_with_more_gas: false,
	empty_considered_exists: false,
	create_increase_nonce: true,
	call_l64_after_gas: true,
	stack_limit: 1024,
	memory_limit: usize::max_value(),
	call_stack_limit: 1024,
	create_contract_limit: Some(MaxCodeSize::get() as usize),
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
	use super::*;

	parameter_types! {
		// Contract max code size.
		pub const MaxCodeSize: u32 = 60 * 1024;
	}

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

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Precompiles associated with this EVM engine.
		type Precompiles: PrecompileSet;

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

		/// EVM execution runner.
		type Runner: Runner<Self>;

		/// Find author for the current block.
		type FindAuthor: FindAuthor<Self::AccountId>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, TypeInfo)]
	pub struct ContractInfo {
		pub code_hash: H256,
		pub maintainer: EvmAddress,
		pub deployed: bool,
	}

	#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, TypeInfo)]
	pub struct AccountInfo<Index> {
		pub nonce: Index,
		pub contract_info: Option<ContractInfo>,
	}

	impl<Index> AccountInfo<Index> {
		pub fn new(nonce: Index, contract_info: Option<ContractInfo>) -> Self {
			Self { nonce, contract_info }
		}
	}

	#[derive(Clone, Copy, Eq, PartialEq, RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo)]
	pub struct CodeInfo {
		pub code_size: u32,
		pub ref_count: u32,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	/// Account definition used for genesis block construction.
	pub struct GenesisAccount<Balance, Index> {
		/// Account nonce.
		pub nonce: Index,
		/// Account balance.
		pub balance: Balance,
		/// Full account storage.
		pub storage: BTreeMap<H256, H256>,
		/// Account code.
		pub code: Vec<u8>,
	}

	/// The EVM accounts info.
	///
	/// Accounts: map EvmAddress => Option<AccountInfo<T>>
	#[pallet::storage]
	#[pallet::getter(fn accounts)]
	pub type Accounts<T: Config> = StorageMap<_, Twox64Concat, EvmAddress, AccountInfo<T::Index>, OptionQuery>;

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
	pub type Codes<T: Config> = StorageMap<_, Identity, H256, BoundedVec<u8, MaxCodeSize>, ValueQuery>;

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
		pub accounts: BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>,
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
			use sp_std::rc::Rc;

			// NOTE: Only applicable for mandala testnet, unit test and integration test.
			// Use create_predeploy_contract to deploy predeploy contracts on the mainnet.
			let source = T::NetworkContractSource::get();

			self.accounts.iter().for_each(|(address, account)| {
				let account_id = T::AddressMapping::get_account_id(address);

				let account_info = <AccountInfo<T::Index>>::new(account.nonce, None);
				<Accounts<T>>::insert(address, account_info);

				let amount = if account.balance.is_zero() {
					T::Currency::minimum_balance()
				} else {
					account.balance
				};
				T::Currency::deposit_creating(&account_id, amount);

				if !account.code.is_empty() {
					// Transactions are not supported by BasicExternalities
					// Use the EVM Runtime
					let vicinity = Vicinity {
						gas_price: U256::one(),
						origin: Default::default(),
					};
					let context = Context {
						caller: source,
						address: *address,
						apparent_value: Default::default(),
					};
					let metadata =
						StackSubstateMetadata::new(210_000, 1000, T::NewContractExtraBytes::get(), T::config());
					let state = SubstrateStackState::<T>::new(&vicinity, metadata);
					let mut executor = StackExecutor::new(state, T::config());

					let mut runtime =
						evm::Runtime::new(Rc::new(account.code.clone()), Rc::new(Vec::new()), context, T::config());
					let reason = executor.execute(&mut runtime);

					assert!(
						reason.is_succeed(),
						"Genesis contract failed to execute, error: {:?}",
						reason
					);

					let out = runtime.machine().return_value();
					<Pallet<T>>::create_contract(source, *address, out);

					#[cfg(not(feature = "with-ethereum-compatibility"))]
					<Pallet<T>>::mark_deployed(*address, None).expect("Genesis contract failed to deploy");

					for (index, value) in &account.storage {
						AccountStorages::<T>::insert(address, index, value);
					}
				}
			});
			NetworkContractIndex::<T>::put(MIRRORED_NFT_ADDRESS_START);
		}
	}

	/// EVM events
	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A contract has been created at given \[from, address, logs\].
		Created(EvmAddress, EvmAddress, Vec<Log>),
		/// A contract was attempted to be created, but the execution failed.
		/// \[from, contract, exit_reason, logs\]
		CreatedFailed(EvmAddress, EvmAddress, ExitReason, Vec<Log>),
		/// A contract has been executed successfully with states applied. \[from, contract, logs]\
		Executed(EvmAddress, EvmAddress, Vec<Log>),
		/// A contract has been executed with errors. States are reverted with
		/// only gas fees applied. \[from, contract, exit_reason, output, logs\]
		ExecutedFailed(EvmAddress, EvmAddress, ExitReason, Vec<u8>, Vec<Log>),
		/// Transferred maintainer. \[contract, address\]
		TransferredMaintainer(EvmAddress, EvmAddress),
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
		/// Contract development is not enabled
		ContractDevelopmentNotEnabled,
		/// Contract development is already enabled
		ContractDevelopmentAlreadyEnabled,
		/// Contract already deployed
		ContractAlreadyDeployed,
		/// Contract exceeds max code size
		ContractExceedsMaxCodeSize,
		/// Contract already existed
		ContractAlreadyExisted,
		/// Storage usage exceeds storage limit
		OutOfStorage,
		/// Charge fee failed
		ChargeFeeFailed,
		/// Contract cannot be killed due to reference count
		CannotKillContract,
		/// Reserve storage failed
		ReserveStorageFailed,
		/// Unreserve storage failed
		UnreserveStorageFailed,
		/// Charge storage failed
		ChargeStorageFailed,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		#[transactional]
		pub fn eth_call(
			origin: OriginFor<T>,
			action: TransactionAction,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			#[pallet::compact] _valid_until: T::BlockNumber, // checked by tx validation logic
		) -> DispatchResultWithPostInfo {
			match action {
				TransactionAction::Call(target) => Self::call(origin, target, input, value, gas_limit, storage_limit),
				TransactionAction::Create => Self::create(origin, input, value, gas_limit, storage_limit),
			}
		}

		/// Issue an EVM call operation. This is similar to a message call
		/// transaction in Ethereum.
		///
		/// - `target`: the contract address to call
		/// - `input`: the data supplied for the call
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		#[transactional]
		pub fn call(
			origin: OriginFor<T>,
			target: EvmAddress,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = T::Runner::call(
				source,
				source,
				target,
				input,
				value,
				gas_limit,
				storage_limit,
				T::config(),
			)?;

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
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
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

			let info = T::Runner::call(from, from, target, input, value, gas_limit, storage_limit, T::config())?;

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
		#[transactional]
		pub fn create(
			origin: OriginFor<T>,
			init: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = T::Runner::create(source, init, value, gas_limit, storage_limit, T::config())?;

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
		#[transactional]
		pub fn create2(
			origin: OriginFor<T>,
			init: Vec<u8>,
			salt: H256,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = T::Runner::create2(source, init, salt, value, gas_limit, storage_limit, T::config())?;

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
		#[transactional]
		pub fn create_network_contract(
			origin: OriginFor<T>,
			init: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			let source = T::NetworkContractSource::get();
			let address = EvmAddress::from_low_u64_be(Self::network_contract_index());
			let info =
				T::Runner::create_at_address(source, address, init, value, gas_limit, storage_limit, T::config())?;

			NetworkContractIndex::<T>::mutate(|v| *v = v.saturating_add(One::one()));

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes,
			})
		}

		/// Issue an EVM create operation. The address specified
		/// will be used as created contract address.
		///
		/// - `target`: the address specified by the contract
		/// - `init`: the data supplied for the contract's constructor
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(T::GasToWeight::convert(*gas_limit))]
		#[transactional]
		pub fn create_predeploy_contract(
			origin: OriginFor<T>,
			target: EvmAddress,
			init: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			ensure!(
				Pallet::<T>::is_account_empty(&target),
				Error::<T>::ContractAlreadyExisted
			);

			let source = T::NetworkContractSource::get();

			let info = if init.is_empty() {
				// deposit ED for mirrored token
				T::Currency::transfer(
					&T::TreasuryAccount::get(),
					&T::AddressMapping::get_account_id(&target),
					T::Currency::minimum_balance(),
					ExistenceRequirement::AllowDeath,
				)?;
				CreateInfo {
					value: target,
					exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
					used_gas: 0.into(),
					used_storage: 0,
					logs: vec![],
				}
			} else {
				T::Runner::create_at_address(source, target, init, value, gas_limit, storage_limit, T::config())?
			};

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
	/// Check whether an account is empty.
	pub fn is_account_empty(address: &H160) -> bool {
		let account_id = T::AddressMapping::get_account_id(address);
		let balance = T::Currency::total_balance(&account_id);

		if !balance.is_zero() {
			return false;
		}

		Self::accounts(address).map_or(true, |account_info| {
			account_info.contract_info.is_none() && account_info.nonce.is_zero()
		})
	}

	/// Remove an account if its empty.
	/// Unused now.
	pub fn remove_account_if_empty(address: &H160) {
		if Self::is_account_empty(address) {
			let res = Self::remove_account(address);
			debug_assert!(res.is_ok());
		}
	}

	#[transactional]
	pub fn remove_contract(address: &EvmAddress) -> Result<u32, DispatchError> {
		let address_account = T::AddressMapping::get_account_id(address);

		let size = Accounts::<T>::try_mutate_exists(address, |account_info| -> Result<u32, DispatchError> {
			let account_info = account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info.contract_info.take().ok_or(Error::<T>::ContractNotFound)?;

			let maintainer_account = T::AddressMapping::get_account_id(&contract_info.maintainer);
			T::TransferAll::transfer_all(&address_account, &maintainer_account)?;

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

			AccountStorages::<T>::remove_prefix(address, None);

			let size = ContractStorageSizes::<T>::take(address);

			Ok(size)
		})?;

		// this should happen after `Accounts` is updated because this could trigger another updates on
		// `Accounts`
		frame_system::Pallet::<T>::dec_providers(&address_account)?;

		Ok(size)
	}

	/// Removes an account from Accounts and AccountStorages.
	pub fn remove_account(address: &EvmAddress) -> DispatchResult {
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

	/// Create an account.
	/// - Create new account for the contract.
	/// - Update codes info.
	/// - Update maintainer of the contract.
	/// - Save `code` if not saved yet.
	pub fn create_contract(source: H160, address: H160, code: Vec<u8>) {
		let bounded_code: BoundedVec<u8, MaxCodeSize> = code
			.try_into()
			.expect("checked by create_contract_limit in ACALA_CONFIG; qed");
		if bounded_code.is_empty() {
			return;
		}

		// if source is account, the maintainer of the new contract is source.
		// if source is contract, the maintainer of the new contract is the maintainer of the contract.
		let maintainer = Self::accounts(source).map_or(source, |account_info| {
			account_info
				.contract_info
				.map_or(source, |contract_info| contract_info.maintainer)
		});

		let code_hash = code_hash(bounded_code.as_slice());
		let code_size = bounded_code.len() as u32;

		let contract_info = ContractInfo {
			code_hash,
			maintainer,
			#[cfg(feature = "with-ethereum-compatibility")]
			deployed: true,
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			deployed: false,
		};

		Self::update_contract_storage_size(
			&address,
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
				let account_info = AccountInfo::<T::Index>::new(Default::default(), Some(contract_info.clone()));
				*maybe_account_info = Some(account_info);
			}
		});

		frame_system::Pallet::<T>::inc_providers(&T::AddressMapping::get_account_id(&address));
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

	/// Get the author using the FindAuthor trait.
	pub fn find_author() -> H160 {
		let digest = <frame_system::Pallet<T>>::digest();
		let pre_runtime_digests = digest.logs.iter().filter_map(|d| d.as_pre_runtime());

		let author = T::FindAuthor::find_author(pre_runtime_digests).unwrap_or_default();
		T::AddressMapping::get_default_evm_address(&author)
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
			// The same as `code_hash(&[])`, hardcode here.
			H256::from_slice(&hex!(
				"c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
			))
		}
	}

	/// Get code at given address.
	pub fn code_at_address(address: &EvmAddress) -> BoundedVec<u8, MaxCodeSize> {
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
				.as_mut()
				.ok_or(Error::<T>::ContractNotFound)?;

			let source = if let Either::Right(signer) = root_or_signed {
				let maintainer = T::AddressMapping::get_evm_address(&signer).ok_or(Error::<T>::AddressNotMapped)?;
				ensure!(contract_info.maintainer == maintainer, Error::<T>::NoPermission);
				ensure!(!contract_info.deployed, Error::<T>::ContractAlreadyDeployed);
				maintainer
			} else {
				T::NetworkContractSource::get()
			};

			let old_code_info = Self::code_infos(&contract_info.code_hash).ok_or(Error::<T>::ContractNotFound)?;

			let bounded_code: BoundedVec<u8, MaxCodeSize> =
				code.try_into().map_err(|_| Error::<T>::ContractExceedsMaxCodeSize)?;
			let code_hash = code_hash(bounded_code.as_slice());
			let code_size = bounded_code.len() as u32;
			// The code_hash of the same contract is definitely different.
			// The `contract_info.code_hash` hashed by on_contract_initialization which constructored.
			// Still check it here.
			if code_hash == contract_info.code_hash {
				return Ok(());
			}

			let storage_size_chainged: i32 =
				code_size.saturating_add(T::NewContractExtraBytes::get()) as i32 - old_code_info.code_size as i32;

			if storage_size_chainged.is_positive() {
				Self::reserve_storage(&source, storage_size_chainged as u32)?;
			}
			Self::charge_storage(&source, &contract, storage_size_chainged)?;
			Self::update_contract_storage_size(&contract, storage_size_chainged);

			// try remove old codes
			CodeInfos::<T>::mutate_exists(&contract_info.code_hash, |maybe_code_info| -> DispatchResult {
				let code_info = maybe_code_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
				code_info.ref_count = code_info.ref_count.saturating_sub(1);
				if code_info.ref_count == 0 {
					Codes::<T>::remove(&contract_info.code_hash);
					*maybe_code_info = None;
				}
				Ok(())
			})?;

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
			// update code_hash
			contract_info.code_hash = code_hash;

			Ok(())
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

		let storage = Self::remove_contract(&contract)?;

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

	fn can_call_contract(address: &H160, caller: &H160) -> bool {
		if let Some(AccountInfo {
			contract_info: Some(ContractInfo {
				deployed, maintainer, ..
			}),
			..
		}) = Accounts::<T>::get(address)
		{
			deployed || maintainer == *caller || Self::is_developer_or_contract(caller)
		} else {
			// contract non exist, we don't override defualt evm behaviour
			true
		}
	}

	fn is_developer_or_contract(caller: &H160) -> bool {
		if let Some(AccountInfo { contract_info, .. }) = Accounts::<T>::get(caller) {
			let account_id = T::AddressMapping::get_account_id(caller);
			contract_info.is_some()
				|| !T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &account_id).is_zero()
		} else {
			false
		}
	}

	fn reserve_storage(caller: &H160, limit: u32) -> DispatchResult {
		if limit.is_zero() {
			return Ok(());
		}

		let user = T::AddressMapping::get_account_id(caller);
		let amount = T::StorageDepositPerByte::get().saturating_mul(limit.into());

		log::debug!(
			target: "evm",
			"reserve_storage: [from: {:?}, account: {:?}, limit: {:?}, amount: {:?}]",
			caller, user, limit, amount
		);

		T::Currency::reserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount)
	}

	fn unreserve_storage(caller: &H160, limit: u32, used: u32, refunded: u32) -> DispatchResult {
		let total = limit.saturating_add(refunded);
		let unused = total.saturating_sub(used);
		if unused.is_zero() {
			return Ok(());
		}

		let user = T::AddressMapping::get_account_id(caller);
		let amount = T::StorageDepositPerByte::get().saturating_mul(unused.into());

		log::debug!(
			target: "evm",
			"unreserve_storage: [from: {:?}, account: {:?}, used: {:?}, refunded: {:?}, unused: {:?}, amount: {:?}]",
			caller, user, used, refunded, unused, amount
		);

		// should always be able to unreserve the amount
		// but otherwise we will just ignore the issue here.
		let err_amount = T::Currency::unreserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount);
		debug_assert!(err_amount.is_zero());
		Ok(())
	}

	fn charge_storage(caller: &H160, contract: &H160, storage: i32) -> DispatchResult {
		if storage.is_zero() {
			return Ok(());
		}

		let user = T::AddressMapping::get_account_id(caller);
		let contract_acc = T::AddressMapping::get_account_id(contract);
		let amount = T::StorageDepositPerByte::get().saturating_mul((storage.abs() as u32).into());

		log::debug!(
			target: "evm",
			"charge_storage: [from: {:?}, account: {:?}, contract: {:?}, contract_acc: {:?}, storage: {:?}, amount: {:?}]",
			caller, user, contract, contract_acc, storage, amount
		);

		if storage.is_positive() {
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
	) -> Result<CallInfo, DispatchError> {
		let mut config = T::config().clone();
		if let ExecutionMode::EstimateGas = mode {
			config.estimate = true;
		}

		frame_support::storage::with_transaction(|| {
			let result = T::Runner::call(
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
							TransactionOutcome::Commit(Ok(info))
						} else {
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

fn encode_revert_message(e: &ExitError) -> Vec<u8> {
	// A minimum size of error function selector (4) + offset (32) + string length
	// (32) should contain a utf-8 encoded revert reason.

	let mut w = sp_std::Writer::default();
	let _ = core::write!(&mut w, "{:?}", e);
	let msg = w.into_inner();

	let mut data = Vec::with_capacity(68 + msg.len());
	data.extend_from_slice(&[0u8; 68]);
	U256::from(msg.len()).to_big_endian(&mut data[36..68]);
	data.extend_from_slice(&msg);
	data
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
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
