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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

pub use crate::runner::{
	stack::SubstrateStackState,
	state::{PrecompileSet, StackExecutor, StackSubstateMetadata},
	storage_meter::StorageMeter,
	Runner,
};
use codec::{Decode, Encode, FullCodec, MaxEncodedLen};
use frame_support::{
	dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo},
	ensure,
	error::BadOrigin,
	log,
	pallet_prelude::*,
	parameter_types,
	traits::{
		BalanceStatus, Currency, EnsureOneOf, EnsureOrigin, ExistenceRequirement, FindAuthor, Get,
		NamedReservableCurrency, OnKilledAccount,
	},
	transactional,
	weights::{Pays, PostDispatchInfo, Weight},
	BoundedVec, RuntimeDebug,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*, EnsureRoot, EnsureSigned};
use hex_literal::hex;
pub use module_evm_utility::{
	ethereum::{AccessListItem, Log, TransactionAction},
	evm::{self, Config as EvmConfig, Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed},
	Account,
};
pub use module_support::{
	AddressMapping, DispatchableTask, EVMManager, ExecutionMode, IdleScheduler, InvokeContext, TransactionPayment,
	EVM as EVMTrait,
};
pub use orml_traits::{currency::TransferAll, MultiCurrency};
use primitive_types::{H160, H256, U256};
pub use primitives::{
	evm::{
		convert_decimals_from_evm, convert_decimals_to_evm, CallInfo, CreateInfo, EvmAddress, ExecutionInfo, Vicinity,
		MIRRORED_NFT_ADDRESS_START, MIRRORED_TOKENS_ADDRESS_START,
	},
	task::TaskResult,
	Balance, CurrencyId, ReserveIdentifier,
};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use sp_io::KillStorageResult::{AllRemoved, SomeRemaining};
use sp_runtime::{
	traits::{Convert, DispatchInfoOf, One, PostDispatchInfoOf, SignedExtension, UniqueSaturatedInto, Zero},
	transaction_validity::TransactionValidityError,
	Either, TransactionOutcome,
};
use sp_std::{cmp, collections::btree_map::BTreeMap, fmt::Debug, marker::PhantomData, prelude::*};

pub mod precompiles;
pub mod runner;

pub mod bench;

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

// Initially based on London hard fork configuration.
static ACALA_CONFIG: EvmConfig = EvmConfig {
	refund_sstore_clears: 0,            // no gas refund
	sstore_gas_metering: false,         // no gas refund
	sstore_revert_under_stipend: false, // ignored
	create_contract_limit: Some(MaxCodeSize::get() as usize),
	..module_evm_utility::evm::Config::london()
};

/// Create an empty contract `contract Empty { }`.
pub const BASE_CREATE_GAS: u64 = 67_066;
/// Call function that just set a storage `function store(uint256 num) public { number = num; }`.
pub const BASE_CALL_GAS: u64 = 43_702;

/// Helper method to calculate `create` weight.
fn create_weight<T: Config>(gas: u64) -> Weight {
	<T as Config>::WeightInfo::create()
		// during `create` benchmark an additional of `BASE_CREATE_GAS` was used
		// so user will be extra charged only for extra gas usage
		.saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}

/// Helper method to calculate `create2` weight.
fn create2_weight<T: Config>(gas: u64) -> Weight {
	<T as Config>::WeightInfo::create2()
		// during `create2` benchmark an additional of `BASE_CREATE_GAS` was used
		// so user will be extra charged only for extra gas usage
		.saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}

/// Helper method to calculate `create_predeploy_contract` weight.
fn create_predeploy_contract<T: Config>(gas: u64) -> Weight {
	<T as Config>::WeightInfo::create_predeploy_contract()
		// during `create_predeploy_contract` benchmark an additional of `BASE_CREATE_GAS`
		// was used so user will be extra charged only for extra gas usage
		.saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}

/// Helper method to calculate `create_nft_contract` weight.
fn create_nft_contract<T: Config>(gas: u64) -> Weight {
	<T as Config>::WeightInfo::create_nft_contract()
		// during `create_nft_contract` benchmark an additional of `BASE_CREATE_GAS`
		// was used so user will be extra charged only for extra gas usage
		.saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}

/// Helper method to calculate `call` weight.
fn call_weight<T: Config>(gas: u64) -> Weight {
	<T as Config>::WeightInfo::call()
		// during `call` benchmark an additional of `BASE_CALL_GAS` was used
		// so user will be extra charged only for extra gas usage
		.saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CALL_GAS)))
}

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
		type Currency: NamedReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = ReserveIdentifier,
			Balance = Balance,
		>;

		/// Merge free balance from source to dest.
		type TransferAll: TransferAll<Self::AccountId>;

		/// Charge extra bytes for creating a contract, would be reserved until
		/// the contract deleted.
		#[pallet::constant]
		type NewContractExtraBytes: Get<u32>;

		/// Storage required for per byte.
		#[pallet::constant]
		type StorageDepositPerByte: Get<BalanceOf<Self>>;

		/// Tx fee required for per gas.
		/// Provide to the client
		#[pallet::constant]
		type TxFeePerGas: Get<BalanceOf<Self>>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Precompiles associated with this EVM engine.
		type PrecompilesType: PrecompileSet;
		type PrecompilesValue: Get<Self::PrecompilesType>;

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

		/// The fee for publishing the contract.
		#[pallet::constant]
		type PublicationFee: Get<BalanceOf<Self>>;

		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		type FreePublicationOrigin: EnsureOrigin<Self::Origin>;

		/// EVM execution runner.
		type Runner: Runner<Self>;

		/// Find author for the current block.
		type FindAuthor: FindAuthor<Self::AccountId>;

		/// Dispatchable tasks
		type Task: DispatchableTask + FullCodec + Debug + Clone + PartialEq + TypeInfo + From<EvmTask<Self>>;

		/// Idle scheduler for the evm task.
		type IdleScheduler: IdleScheduler<Self::Task>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, TypeInfo)]
	pub struct ContractInfo {
		pub code_hash: H256,
		pub maintainer: EvmAddress,
		pub published: bool,
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

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo, Default)]
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
		/// If the account should enable contract development mode
		pub enable_contract_development: bool,
	}

	/// The EVM Chain ID.
	///
	/// ChainId: u64
	#[pallet::storage]
	#[pallet::getter(fn chain_id)]
	pub type ChainId<T: Config> = StorageValue<_, u64, ValueQuery>;

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
		pub chain_id: u64,
		pub accounts: BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				chain_id: Default::default(),
				accounts: Default::default(),
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
					<T::Currency as Currency<T::AccountId>>::minimum_balance()
				} else {
					account.balance
				};
				T::Currency::deposit_creating(&account_id, amount);

				if account.enable_contract_development {
					T::Currency::ensure_reserved_named(
						&RESERVE_ID_DEVELOPER_DEPOSIT,
						&account_id,
						T::DeveloperDeposit::get(),
					)
					.expect("Failed to reserve developer deposit. Please make sure the account have enough balance.");
				}

				if !account.code.is_empty() {
					// init contract

					// Transactions are not supported by BasicExternalities
					// Use the EVM Runtime
					let vicinity = Vicinity {
						gas_price: U256::one(),
						..Default::default()
					};
					let context = Context {
						caller: source,
						address: *address,
						apparent_value: Default::default(),
					};
					let metadata = StackSubstateMetadata::new(210_000, 1000, T::config());
					let state = SubstrateStackState::<T>::new(&vicinity, metadata);
					let mut executor = StackExecutor::new_with_precompiles(state, T::config(), &());

					let mut runtime =
						evm::Runtime::new(Rc::new(account.code.clone()), Rc::new(Vec::new()), context, T::config());
					let reason = executor.execute(&mut runtime);

					assert!(
						reason.is_succeed(),
						"Genesis contract failed to execute, error: {:?}",
						reason
					);

					let out = runtime.machine().return_value();
					<Pallet<T>>::create_contract(source, *address, true, out);

					for (index, value) in &account.storage {
						AccountStorages::<T>::insert(address, index, value);
					}
				}
			});
			ChainId::<T>::put(self.chain_id);
			NetworkContractIndex::<T>::put(MIRRORED_NFT_ADDRESS_START);
		}
	}

	/// EVM events
	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A contract has been created at given
		Created {
			from: EvmAddress,
			contract: EvmAddress,
			logs: Vec<Log>,
			used_gas: u64,
			used_storage: i32,
		},
		/// A contract was attempted to be created, but the execution failed.
		CreatedFailed {
			from: EvmAddress,
			contract: EvmAddress,
			exit_reason: ExitReason,
			logs: Vec<Log>,
			used_gas: u64,
			used_storage: i32,
		},
		/// A contract has been executed successfully with states applied.
		Executed {
			from: EvmAddress,
			contract: EvmAddress,
			logs: Vec<Log>,
			used_gas: u64,
			used_storage: i32,
		},
		/// A contract has been executed with errors. States are reverted with
		/// only gas fees applied.
		ExecutedFailed {
			from: EvmAddress,
			contract: EvmAddress,
			exit_reason: ExitReason,
			output: Vec<u8>,
			logs: Vec<Log>,
			used_gas: u64,
			used_storage: i32,
		},
		/// Transferred maintainer.
		TransferredMaintainer {
			contract: EvmAddress,
			new_maintainer: EvmAddress,
		},
		/// Enabled contract development.
		ContractDevelopmentEnabled { who: T::AccountId },
		/// Disabled contract development.
		ContractDevelopmentDisabled { who: T::AccountId },
		/// Published contract.
		ContractPublished { contract: EvmAddress },
		/// Set contract code.
		ContractSetCode { contract: EvmAddress },
		/// Selfdestructed contract code.
		ContractSelfdestructed { contract: EvmAddress },
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
		/// Contract already published
		ContractAlreadyPublished,
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
		/// Invalid decimals
		InvalidDecimals,
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn integrity_test() {
			assert!(convert_decimals_from_evm(T::StorageDepositPerByte::get()).is_some());
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(match *action {
			TransactionAction::Call(_) => call_weight::<T>(*gas_limit),
			TransactionAction::Create => create_weight::<T>(*gas_limit)
		})]
		#[transactional]
		pub fn eth_call(
			origin: OriginFor<T>,
			action: TransactionAction,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			access_list: Vec<AccessListItem>,
			#[pallet::compact] _valid_until: T::BlockNumber, // checked by tx validation logic
		) -> DispatchResultWithPostInfo {
			match action {
				TransactionAction::Call(target) => {
					Self::call(origin, target, input, value, gas_limit, storage_limit, access_list)
				}
				TransactionAction::Create => Self::create(origin, input, value, gas_limit, storage_limit, access_list),
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
		#[pallet::weight(call_weight::<T>(*gas_limit))]
		#[transactional]
		pub fn call(
			origin: OriginFor<T>,
			target: EvmAddress,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			access_list: Vec<AccessListItem>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			match T::Runner::call(
				source,
				source,
				target,
				input,
				value,
				gas_limit,
				storage_limit,
				access_list.into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				T::config(),
			) {
				Err(e) => {
					Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed {
						from: source,
						contract: target,
						exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(e).into())),
						output: vec![],
						logs: vec![],
						used_gas: gas_limit,
						used_storage: Default::default(),
					});

					Ok(().into())
				}
				Ok(info) => {
					let used_gas: u64 = info.used_gas.unique_saturated_into();

					if info.exit_reason.is_succeed() {
						Pallet::<T>::deposit_event(Event::<T>::Executed {
							from: source,
							contract: target,
							logs: info.logs,
							used_gas,
							used_storage: info.used_storage,
						});
					} else {
						Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed {
							from: source,
							contract: target,
							exit_reason: info.exit_reason.clone(),
							output: info.value.clone(),
							logs: info.logs,
							used_gas,
							used_storage: Default::default(),
						});
					}

					Ok(PostDispatchInfo {
						actual_weight: Some(call_weight::<T>(used_gas)),
						pays_fee: Pays::Yes,
					})
				}
			}
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
		// TODO: create benchmark
		pub fn scheduled_call(
			origin: OriginFor<T>,
			from: EvmAddress,
			target: EvmAddress,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			access_list: Vec<AccessListItem>,
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

			match T::Runner::call(
				from,
				from,
				target,
				input,
				value,
				gas_limit,
				storage_limit,
				access_list.into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				T::config(),
			) {
				Err(e) => {
					Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed {
						from,
						contract: target,
						exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(e).into())),
						output: vec![],
						logs: vec![],
						used_gas: gas_limit,
						used_storage: Default::default(),
					});

					Ok(().into())
				}
				Ok(info) => {
					let used_gas: u64 = info.used_gas.unique_saturated_into();

					if info.exit_reason.is_succeed() {
						Pallet::<T>::deposit_event(Event::<T>::Executed {
							from,
							contract: target,
							logs: info.logs,
							used_gas,
							used_storage: info.used_storage,
						});
					} else {
						Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed {
							from,
							contract: target,
							exit_reason: info.exit_reason.clone(),
							output: info.value.clone(),
							logs: info.logs,
							used_gas,
							used_storage: Default::default(),
						});
					}

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
			}
		}

		/// Issue an EVM create operation. This is similar to a contract
		/// creation transaction in Ethereum.
		///
		/// - `input`: the data supplied for the contract's constructor
		/// - `value`: the amount sent to the contract upon creation
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(create_weight::<T>(*gas_limit))]
		#[transactional]
		pub fn create(
			origin: OriginFor<T>,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			access_list: Vec<AccessListItem>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			match T::Runner::create(
				source,
				input,
				value,
				gas_limit,
				storage_limit,
				access_list.into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				T::config(),
			) {
				Err(e) => {
					Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
						from: source,
						contract: H160::default(),
						exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(e).into())),
						logs: vec![],
						used_gas: gas_limit,
						used_storage: Default::default(),
					});

					Ok(().into())
				}
				Ok(info) => {
					let used_gas: u64 = info.used_gas.unique_saturated_into();

					if info.exit_reason.is_succeed() {
						Pallet::<T>::deposit_event(Event::<T>::Created {
							from: source,
							contract: info.value,
							logs: info.logs,
							used_gas,
							used_storage: info.used_storage,
						});
					} else {
						Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
							from: source,
							contract: info.value,
							exit_reason: info.exit_reason.clone(),
							logs: info.logs,
							used_gas,
							used_storage: Default::default(),
						});
					}

					Ok(PostDispatchInfo {
						actual_weight: Some(create_weight::<T>(used_gas)),
						pays_fee: Pays::Yes,
					})
				}
			}
		}

		/// Issue an EVM create2 operation.
		///
		/// - `target`: the contract address to call
		/// - `input`: the data supplied for the contract's constructor
		/// - `salt`: used for generating the new contract's address
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(create2_weight::<T>(*gas_limit))]
		#[transactional]
		pub fn create2(
			origin: OriginFor<T>,
			input: Vec<u8>,
			salt: H256,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			access_list: Vec<AccessListItem>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			match T::Runner::create2(
				source,
				input,
				salt,
				value,
				gas_limit,
				storage_limit,
				access_list.into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				T::config(),
			) {
				Err(e) => {
					Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
						from: source,
						contract: H160::default(),
						exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(e).into())),
						logs: vec![],
						used_gas: gas_limit,
						used_storage: Default::default(),
					});

					Ok(().into())
				}
				Ok(info) => {
					let used_gas: u64 = info.used_gas.unique_saturated_into();

					if info.exit_reason.is_succeed() {
						Pallet::<T>::deposit_event(Event::<T>::Created {
							from: source,
							contract: info.value,
							logs: info.logs,
							used_gas,
							used_storage: info.used_storage,
						});
					} else {
						Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
							from: source,
							contract: info.value,
							exit_reason: info.exit_reason.clone(),
							logs: info.logs,
							used_gas,
							used_storage: Default::default(),
						});
					}

					Ok(PostDispatchInfo {
						actual_weight: Some(create2_weight::<T>(used_gas)),
						pays_fee: Pays::Yes,
					})
				}
			}
		}

		/// Create mirrored NFT contract. The next available system contract
		/// address will be used as created contract address.
		///
		/// - `input`: the data supplied for the contract's constructor
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(create_nft_contract::<T>(*gas_limit))]
		#[transactional]
		pub fn create_nft_contract(
			origin: OriginFor<T>,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			access_list: Vec<AccessListItem>,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			let source = T::NetworkContractSource::get();
			let source_account = T::AddressMapping::get_account_id(&source);
			let address = MIRRORED_TOKENS_ADDRESS_START | EvmAddress::from_low_u64_be(Self::network_contract_index());

			// ensure source have more than 10 KAR/ACA to deploy the contract.
			let amount = T::Currency::minimum_balance().saturating_mul(100u32.into());
			if T::Currency::free_balance(&source_account) < amount {
				T::Currency::transfer(
					&T::TreasuryAccount::get(),
					&source_account,
					amount,
					ExistenceRequirement::AllowDeath,
				)?;
			}

			match T::Runner::create_at_address(
				source,
				address,
				input,
				value,
				gas_limit,
				storage_limit,
				access_list.into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				T::config(),
			) {
				Err(e) => {
					Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
						from: source,
						contract: H160::default(),
						exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(e).into())),
						logs: vec![],
						used_gas: gas_limit,
						used_storage: Default::default(),
					});

					Ok(().into())
				}
				Ok(info) => {
					let used_gas: u64 = info.used_gas.unique_saturated_into();

					if info.exit_reason.is_succeed() {
						NetworkContractIndex::<T>::mutate(|v| *v = v.saturating_add(One::one()));

						Pallet::<T>::deposit_event(Event::<T>::Created {
							from: source,
							contract: info.value,
							logs: info.logs,
							used_gas,
							used_storage: info.used_storage,
						});
					} else {
						Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
							from: source,
							contract: info.value,
							exit_reason: info.exit_reason.clone(),
							logs: info.logs,
							used_gas,
							used_storage: Default::default(),
						});
					}

					Ok(PostDispatchInfo {
						actual_weight: Some(create_nft_contract::<T>(used_gas)),
						pays_fee: Pays::No,
					})
				}
			}
		}

		/// Issue an EVM create operation. The address specified
		/// will be used as created contract address.
		///
		/// - `target`: the address specified by the contract
		/// - `input`: the data supplied for the contract's constructor
		/// - `value`: the amount sent for payable calls
		/// - `gas_limit`: the maximum gas the call can use
		/// - `storage_limit`: the total bytes the contract's storage can increase by
		#[pallet::weight(create_predeploy_contract::<T>(*gas_limit))]
		#[transactional]
		pub fn create_predeploy_contract(
			origin: OriginFor<T>,
			target: EvmAddress,
			input: Vec<u8>,
			#[pallet::compact] value: BalanceOf<T>,
			#[pallet::compact] gas_limit: u64,
			#[pallet::compact] storage_limit: u32,
			access_list: Vec<AccessListItem>,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			ensure!(Self::accounts(target).is_none(), Error::<T>::ContractAlreadyExisted);

			let source = T::NetworkContractSource::get();
			let source_account = T::AddressMapping::get_account_id(&source);
			// ensure source have more than 10 KAR/ACA to deploy the contract.
			let amount = T::Currency::minimum_balance().saturating_mul(100u32.into());
			if T::Currency::free_balance(&source_account) < amount {
				T::Currency::transfer(
					&T::TreasuryAccount::get(),
					&source_account,
					amount,
					ExistenceRequirement::AllowDeath,
				)?;
			}

			match T::Runner::create_at_address(
				source,
				target,
				input,
				value,
				gas_limit,
				storage_limit,
				access_list.into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				T::config(),
			) {
				Err(e) => {
					Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
						from: source,
						contract: H160::default(),
						exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(e).into())),
						logs: vec![],
						used_gas: gas_limit,
						used_storage: Default::default(),
					});

					Ok(().into())
				}
				Ok(info) => {
					let used_gas: u64 = info.used_gas.unique_saturated_into();
					let contract = info.value;

					if info.exit_reason.is_succeed() {
						Pallet::<T>::deposit_event(Event::<T>::Created {
							from: source,
							contract,
							logs: info.logs,
							used_gas,
							used_storage: info.used_storage,
						});
					} else {
						Pallet::<T>::deposit_event(Event::<T>::CreatedFailed {
							from: source,
							contract,
							exit_reason: info.exit_reason.clone(),
							logs: info.logs,
							used_gas,
							used_storage: Default::default(),
						});
					}

					if info.exit_reason.is_succeed() {
						Self::mark_published(contract, Some(source))?;
						Pallet::<T>::deposit_event(Event::<T>::ContractPublished { contract });
					}

					Ok(PostDispatchInfo {
						actual_weight: Some(create_predeploy_contract::<T>(used_gas)),
						pays_fee: Pays::No,
					})
				}
			}
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

			Pallet::<T>::deposit_event(Event::<T>::TransferredMaintainer {
				contract,
				new_maintainer,
			});

			Ok(().into())
		}

		/// Mark a given contract as published.
		///
		/// - `contract`: The contract to mark as published, the caller must the contract's
		///   maintainer
		#[pallet::weight(<T as Config>::WeightInfo::publish_contract())]
		#[transactional]
		pub fn publish_contract(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_publish_contract(who, contract)?;

			Pallet::<T>::deposit_event(Event::<T>::ContractPublished { contract });
			Ok(().into())
		}

		/// Mark a given contract as published without paying the publication fee
		///
		/// - `contract`: The contract to mark as published, the caller must be the contract's
		///   maintainer.
		#[pallet::weight(<T as Config>::WeightInfo::publish_free())]
		#[transactional]
		pub fn publish_free(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResultWithPostInfo {
			T::FreePublicationOrigin::ensure_origin(origin)?;
			Self::mark_published(contract, None)?;
			Pallet::<T>::deposit_event(Event::<T>::ContractPublished { contract });
			Ok(().into())
		}

		/// Mark the caller's address to allow contract development.
		/// This allows the address to interact with non-published contracts.
		#[pallet::weight(<T as Config>::WeightInfo::enable_contract_development())]
		#[transactional]
		pub fn enable_contract_development(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_enable_contract_development(&who)?;

			Pallet::<T>::deposit_event(Event::<T>::ContractDevelopmentEnabled { who });
			Ok(().into())
		}

		/// Mark the caller's address to disable contract development.
		/// This disallows the address to interact with non-published contracts.
		#[pallet::weight(<T as Config>::WeightInfo::disable_contract_development())]
		#[transactional]
		pub fn disable_contract_development(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_disable_contract_development(&who)?;

			Pallet::<T>::deposit_event(Event::<T>::ContractDevelopmentDisabled { who });
			Ok(().into())
		}

		/// Set the code of a contract at a given address.
		///
		/// - `contract`: The contract whose code is being set, must not be marked as published
		/// - `code`: The new ABI bundle for the contract
		#[pallet::weight(<T as Config>::WeightInfo::set_code(code.len() as u32))]
		#[transactional]
		pub fn set_code(origin: OriginFor<T>, contract: EvmAddress, code: Vec<u8>) -> DispatchResultWithPostInfo {
			let root_or_signed = Self::ensure_root_or_signed(origin)?;
			Self::do_set_code(root_or_signed, contract, code)?;

			Pallet::<T>::deposit_event(Event::<T>::ContractSetCode { contract });

			Ok(().into())
		}

		/// Remove a contract at a given address.
		///
		/// - `contract`: The contract to remove, must not be marked as published
		#[pallet::weight(<T as Config>::WeightInfo::selfdestruct())]
		#[transactional]
		pub fn selfdestruct(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let caller = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
			Self::do_selfdestruct(&caller, &contract)?;

			Pallet::<T>::deposit_event(Event::<T>::ContractSelfdestructed { contract });

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Get StorageDepositPerByte of actual decimals
	pub fn get_storage_deposit_per_byte() -> BalanceOf<T> {
		// StorageDepositPerByte decimals is 18, KAR/ACA decimals is 12, convert to 12 here.
		convert_decimals_from_evm(T::StorageDepositPerByte::get()).expect("checked in integrity_test; qed")
	}

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
	/// Keep the non-zero nonce exists.
	pub fn remove_account_if_empty(address: &H160) {
		if Self::is_account_empty(address) {
			let res = Self::remove_account(address);
			debug_assert!(res.is_ok());
		}
	}

	#[transactional]
	pub fn remove_contract(caller: &EvmAddress, contract: &EvmAddress) -> DispatchResult {
		let contract_account = T::AddressMapping::get_account_id(contract);

		Accounts::<T>::try_mutate_exists(contract, |account_info| -> DispatchResult {
			// We will keep the nonce until the storages are cleared.
			// Only remove the `contract_info`
			let account_info = account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info.contract_info.take().ok_or(Error::<T>::ContractNotFound)?;

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

			ContractStorageSizes::<T>::take(contract);

			T::IdleScheduler::schedule(
				EvmTask::Remove {
					caller: *caller,
					contract: *contract,
					maintainer: contract_info.maintainer,
				}
				.into(),
			)
		})?;

		// this should happen after `Accounts` is updated because this could trigger another updates on
		// `Accounts`
		frame_system::Pallet::<T>::dec_providers(&contract_account)?;

		Ok(())
	}

	/// Removes an account from Accounts and AccountStorages.
	/// Only used in `remove_account_if_empty`
	fn remove_account(address: &EvmAddress) -> DispatchResult {
		// Deref code, and remove it if ref count is zero.
		Accounts::<T>::mutate_exists(&address, |maybe_account| {
			if let Some(account) = maybe_account {
				if let Some(ContractInfo { code_hash, .. }) = account.contract_info {
					CodeInfos::<T>::mutate_exists(&code_hash, |maybe_code_info| {
						if let Some(code_info) = maybe_code_info {
							code_info.ref_count = code_info.ref_count.saturating_sub(1);
							if code_info.ref_count == 0 {
								Codes::<T>::remove(&code_hash);
								*maybe_code_info = None;
							}
						}
					});

					// remove_account can only be called when account is killed. i.e. providers == 0
					// but contract_info should maintain a provider
					// so this should never happen
					log::warn!(
						target: "evm",
						"remove_account: removed account {:?} while is still linked to contract info",
						address
					);
					debug_assert!(false, "removed account while is still linked to contract info");
				}

				*maybe_account = None;
			}
		});

		Ok(())
	}

	/// Create an account.
	/// - Create new account for the contract.
	/// - Update codes info.
	/// - Update maintainer of the contract.
	/// - Save `code` if not saved yet.
	pub fn create_contract(source: H160, address: H160, publish: bool, code: Vec<u8>) {
		let bounded_code: BoundedVec<u8, MaxCodeSize> = code
			.try_into()
			.expect("checked by create_contract_limit in ACALA_CONFIG; qed");
		if bounded_code.is_empty() {
			return;
		}

		// if source is account, the maintainer of the new contract is source.
		// if source is contract, the maintainer of the new contract is the source contract.
		let maintainer = source;
		let code_hash = code_hash(bounded_code.as_slice());
		let code_size = bounded_code.len() as u32;

		let contract_info = ContractInfo {
			code_hash,
			maintainer,
			#[cfg(feature = "with-ethereum-compatibility")]
			published: true,
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			published: publish,
		};

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
			balance: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(
				convert_decimals_to_evm(balance),
			)),
		}
	}

	/// Get the author using the FindAuthor trait.
	pub fn find_author() -> H160 {
		let digest = <frame_system::Pallet<T>>::digest();
		let pre_runtime_digests = digest.logs.iter().filter_map(|d| d.as_pre_runtime());

		if let Some(author) = T::FindAuthor::find_author(pre_runtime_digests) {
			T::AddressMapping::get_default_evm_address(&author)
		} else {
			H160::default()
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

	pub fn is_contract(address: &EvmAddress) -> bool {
		matches!(
			Self::accounts(address),
			Some(AccountInfo {
				contract_info: Some(_),
				..
			})
		)
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

	/// Puts a deposit down to allow account to interact with non-published contracts
	fn do_enable_contract_development(who: &T::AccountId) -> DispatchResult {
		ensure!(
			T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, who).is_zero(),
			Error::<T>::ContractDevelopmentAlreadyEnabled
		);
		T::Currency::ensure_reserved_named(&RESERVE_ID_DEVELOPER_DEPOSIT, who, T::DeveloperDeposit::get())?;
		Ok(())
	}

	/// Returns deposit and disables account for contract development
	fn do_disable_contract_development(who: &T::AccountId) -> DispatchResult {
		ensure!(
			!T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, who).is_zero(),
			Error::<T>::ContractDevelopmentNotEnabled
		);
		T::Currency::unreserve_all_named(&RESERVE_ID_DEVELOPER_DEPOSIT, who);
		Ok(())
	}

	/// Publishes the Contract
	///
	/// Checks that `who` is the contract maintainer and takes the publication fee
	fn do_publish_contract(who: T::AccountId, contract: EvmAddress) -> DispatchResult {
		let address = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
		T::Currency::transfer(
			&who,
			&T::TreasuryAccount::get(),
			T::PublicationFee::get(),
			ExistenceRequirement::AllowDeath,
		)?;
		Self::mark_published(contract, Some(address))?;
		Ok(())
	}

	/// Mark contract as published
	///
	/// If maintainer is provider then it will check maintainer
	fn mark_published(contract: EvmAddress, maintainer: Option<EvmAddress>) -> DispatchResult {
		Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
			if let Some(AccountInfo {
				contract_info: Some(contract_info),
				..
			}) = maybe_account_info.as_mut()
			{
				if let Some(maintainer) = maintainer {
					ensure!(contract_info.maintainer == maintainer, Error::<T>::NoPermission);
				}
				ensure!(!contract_info.published, Error::<T>::ContractAlreadyPublished);
				contract_info.published = true;
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
	/// - Save `code` if not saved yet.
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
				ensure!(!contract_info.published, Error::<T>::ContractAlreadyPublished);
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
			// The `contract_info.code_hash` hashed by on_contract_initialization which constructed.
			// Still check it here.
			if code_hash == contract_info.code_hash {
				return Ok(());
			}

			let storage_size_changed: i32 =
				code_size.saturating_add(T::NewContractExtraBytes::get()) as i32 - old_code_info.code_size as i32;

			if storage_size_changed.is_positive() {
				Self::reserve_storage(&source, storage_size_changed as u32)?;
			}
			Self::charge_storage(&source, &contract, storage_size_changed)?;
			Self::update_contract_storage_size(&contract, storage_size_changed);

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
	fn do_selfdestruct(caller: &EvmAddress, contract: &EvmAddress) -> DispatchResult {
		let account_info = Self::accounts(contract).ok_or(Error::<T>::ContractNotFound)?;
		let contract_info = account_info
			.contract_info
			.as_ref()
			.ok_or(Error::<T>::ContractNotFound)?;

		ensure!(contract_info.maintainer == *caller, Error::<T>::NoPermission);
		ensure!(!contract_info.published, Error::<T>::ContractAlreadyPublished);

		Self::remove_contract(caller, contract)
	}

	fn ensure_root_or_signed(o: T::Origin) -> Result<Either<(), T::AccountId>, BadOrigin> {
		EnsureOneOf::<EnsureRoot<T::AccountId>, EnsureSigned<T::AccountId>>::try_origin(o).map_or(Err(BadOrigin), Ok)
	}

	fn can_call_contract(address: &H160, caller: &H160) -> bool {
		if let Some(AccountInfo {
			contract_info: Some(ContractInfo {
				published, maintainer, ..
			}),
			..
		}) = Accounts::<T>::get(address)
		{
			// https://github.com/AcalaNetwork/Acala/blob/af1c277/modules/evm/rpc/src/lib.rs#L176
			// when rpc is called, from is empty, allowing the call
			published || maintainer == *caller || Self::is_developer_or_contract(caller) || *caller == H160::default()
		} else {
			// contract non exist, we don't override default evm behaviour
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
		let amount = Self::get_storage_deposit_per_byte().saturating_mul(limit.into());

		log::debug!(
			target: "evm",
			"reserve_storage: [from: {:?}, account: {:?}, limit: {:?}, amount: {:?}]",
			caller, user, limit, amount
		);

		T::ChargeTransactionPayment::reserve_fee(&user, amount, Some(RESERVE_ID_STORAGE_DEPOSIT))?;
		Ok(())
	}

	fn unreserve_storage(caller: &H160, limit: u32, used: u32, refunded: u32) -> DispatchResult {
		let total = limit.saturating_add(refunded);
		let unused = total.saturating_sub(used);
		if unused.is_zero() {
			return Ok(());
		}

		let user = T::AddressMapping::get_account_id(caller);
		let amount = Self::get_storage_deposit_per_byte().saturating_mul(unused.into());

		log::debug!(
			target: "evm",
			"unreserve_storage: [from: {:?}, account: {:?}, used: {:?}, refunded: {:?}, unused: {:?}, amount: {:?}]",
			caller, user, used, refunded, unused, amount
		);

		// should always be able to unreserve the amount
		// but otherwise we will just ignore the issue here.
		let err_amount = T::ChargeTransactionPayment::unreserve_fee(&user, amount, Some(RESERVE_ID_STORAGE_DEPOSIT));
		debug_assert!(err_amount.is_zero());
		Ok(())
	}

	fn charge_storage(caller: &H160, contract: &H160, storage: i32) -> DispatchResult {
		if storage.is_zero() {
			return Ok(());
		}

		let user = T::AddressMapping::get_account_id(caller);
		let contract_acc = T::AddressMapping::get_account_id(contract);
		let amount = Self::get_storage_deposit_per_byte().saturating_mul(storage.unsigned_abs().into());

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

	fn refund_storage(caller: &H160, contract: &H160, maintainer: &H160) -> DispatchResult {
		let user = T::AddressMapping::get_account_id(caller);
		let contract_acc = T::AddressMapping::get_account_id(contract);
		let maintainer_acc = T::AddressMapping::get_account_id(maintainer);
		let amount = T::Currency::reserved_balance_named(&RESERVE_ID_STORAGE_DEPOSIT, &contract_acc);

		log::debug!(
			target: "evm",
			"refund_storage: [from: {:?}, account: {:?}, contract: {:?}, contract_acc: {:?}, maintainer: {:?}, maintainer_acc: {:?}, amount: {:?}]",
			caller, user, contract, contract_acc, maintainer, maintainer_acc, amount
		);

		// user can't be a dead account
		let val = T::Currency::repatriate_reserved_named(
			&RESERVE_ID_STORAGE_DEPOSIT,
			&contract_acc,
			&user,
			amount,
			BalanceStatus::Free,
		)?;
		debug_assert!(val.is_zero());

		T::TransferAll::transfer_all(&contract_acc, &maintainer_acc)?;

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
				vec![],
				&config,
			);

			match result {
				Ok(info) => match mode {
					ExecutionMode::Execute => {
						if info.exit_reason.is_succeed() {
							Pallet::<T>::deposit_event(Event::<T>::Executed {
								from: context.sender,
								contract: context.contract,
								logs: info.logs.clone(),
								used_gas: info.used_gas.unique_saturated_into(),
								used_storage: info.used_storage,
							});
							TransactionOutcome::Commit(Ok(info))
						} else {
							Pallet::<T>::deposit_event(Event::<T>::ExecutedFailed {
								from: context.sender,
								contract: context.contract,
								exit_reason: info.exit_reason.clone(),
								output: info.value.clone(),
								logs: info.logs.clone(),
								used_gas: info.used_gas.unique_saturated_into(),
								used_storage: Default::default(),
							});
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

pub struct EvmChainId<T>(PhantomData<T>);
impl<T: Config> Get<u64> for EvmChainId<T> {
	fn get() -> u64 {
		Pallet::<T>::chain_id()
	}
}

impl<T: Config> EVMManager<T::AccountId, BalanceOf<T>> for Pallet<T> {
	fn query_new_contract_extra_bytes() -> u32 {
		T::NewContractExtraBytes::get()
	}

	fn query_storage_deposit_per_byte() -> BalanceOf<T> {
		// the decimals is already 18
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
		convert_decimals_to_evm(T::DeveloperDeposit::get())
	}

	fn query_publication_fee() -> BalanceOf<T> {
		convert_decimals_to_evm(T::PublicationFee::get())
	}

	fn transfer_maintainer(from: T::AccountId, contract: EvmAddress, new_maintainer: EvmAddress) -> DispatchResult {
		Pallet::<T>::do_transfer_maintainer(from, contract, new_maintainer)
	}

	fn publish_contract_precompile(who: T::AccountId, contract: H160) -> DispatchResult {
		Pallet::<T>::do_publish_contract(who, contract)
	}

	fn query_developer_status(who: T::AccountId) -> bool {
		!T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &who).is_zero()
	}

	fn enable_account_contract_development(who: T::AccountId) -> DispatchResult {
		Pallet::<T>::do_enable_contract_development(&who)
	}

	fn disable_account_contract_development(who: T::AccountId) -> sp_runtime::DispatchResult {
		Pallet::<T>::do_disable_contract_development(&who)
	}
}

pub struct CallKillAccount<T>(PhantomData<T>);
impl<T: Config> OnKilledAccount<T::AccountId> for CallKillAccount<T> {
	fn on_killed_account(who: &T::AccountId) {
		if let Some(address) = T::AddressMapping::get_evm_address(who) {
			Pallet::<T>::remove_account_if_empty(&address);
		}
	}
}

pub fn code_hash(code: &[u8]) -> H256 {
	H256::from_slice(Keccak256::digest(code).as_slice())
}

#[allow(dead_code)]
fn encode_revert_message(msg: &[u8]) -> Vec<u8> {
	// A minimum size of error function selector (4) + offset (32) + string length
	// (32) should contain a utf-8 encoded revert reason.
	let mut data = Vec::with_capacity(68 + msg.len());
	data.extend_from_slice(&[0u8; 68]);
	U256::from(msg.len()).to_big_endian(&mut data[36..68]);
	data.extend_from_slice(msg);
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
		_pre: Option<Self::Pre>,
		_info: &DispatchInfoOf<Self::Call>,
		_post_info: &PostDispatchInfoOf<Self::Call>,
		_len: usize,
		_result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		ExtrinsicOrigin::<T>::kill();
		Ok(())
	}
}

#[derive(Clone, RuntimeDebug, PartialEq, Encode, Decode, TypeInfo)]
pub enum EvmTask<T: Config> {
	// TODO: update
	Schedule {
		from: EvmAddress,
		target: EvmAddress,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
	},
	Remove {
		caller: EvmAddress,
		contract: EvmAddress,
		maintainer: EvmAddress,
	},
}

impl<T: Config> DispatchableTask for EvmTask<T> {
	fn dispatch(self, weight: Weight) -> TaskResult {
		match self {
			// TODO: update
			EvmTask::Schedule { .. } => {
				// check weight and call `scheduled_call`
				TaskResult {
					result: Ok(()),
					used_weight: 0,
					finished: false,
				}
			}
			EvmTask::Remove {
				caller,
				contract,
				maintainer,
			} => {
				// default limit 100
				let limit = cmp::min(
					weight
						.checked_div(<T as frame_system::Config>::DbWeight::get().write)
						.unwrap_or(100),
					100,
				) as u32;

				match <AccountStorages<T>>::remove_prefix(contract, Some(limit)) {
					AllRemoved(count) => {
						let res = Pallet::<T>::refund_storage(&caller, &contract, &maintainer);
						log::debug!(
							target: "evm",
							"EvmTask::Remove: [from: {:?}, contract: {:?}, maintainer: {:?}, count: {:?}, result: {:?}]",
							caller, contract, maintainer, count, res
						);

						// Remove account after all of the storages are cleared.
						Pallet::<T>::remove_account_if_empty(&contract);

						TaskResult {
							result: res,
							used_weight: <T as frame_system::Config>::DbWeight::get()
								.write
								.saturating_mul(count.into()),
							finished: true,
						}
					}
					SomeRemaining(count) => {
						log::debug!(
							target: "evm",
							"EvmTask::Remove: [from: {:?}, contract: {:?}, maintainer: {:?}, count: {:?}]",
							caller, contract, maintainer, count
						);

						TaskResult {
							result: Ok(()),
							used_weight: <T as frame_system::Config>::DbWeight::get()
								.write
								.saturating_mul(count.into()),
							finished: false,
						}
					}
				}
			}
		}
	}
}

#[cfg(feature = "std")]
impl<T: Config> From<EvmTask<T>> for () {
	fn from(_task: EvmTask<T>) -> Self {
		unimplemented!()
	}
}
