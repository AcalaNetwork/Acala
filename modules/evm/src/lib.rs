#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::or_fun_call)]

pub mod precompiles;
pub mod runner;

mod mock;
mod tests;

pub use crate::precompiles::{Precompile, Precompiles};
pub use crate::runner::Runner;
pub use evm::{Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed};
pub use primitives::evm::{Account, CallInfo, CreateInfo, Log, Vicinity};

use codec::{Decode, Encode};
use evm::Config;
use frame_support::dispatch::DispatchResultWithPostInfo;
use frame_support::traits::{Currency, EnsureOrigin, Get, OnKilledAccount, ReservableCurrency};
use frame_support::weights::{Pays, PostDispatchInfo, Weight};
use frame_support::RuntimeDebug;
use frame_support::{decl_error, decl_event, decl_module, decl_storage};
use frame_system::ensure_signed;
use orml_traits::account::MergeAccount;
use primitives::evm::AddressMapping;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{Convert, One, UniqueSaturatedInto};
use sp_std::{marker::PhantomData, vec::Vec};
use support::EVM as EVMTrait;

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

/// Substrate system chain ID.
pub struct SystemChainId;

impl Get<u64> for SystemChainId {
	fn get() -> u64 {
		sp_io::misc::chain_id()
	}
}

// Initially based on Istanbul hard fork configuration.
static ACALA_CONFIG: Config = Config {
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

/// EVM module trait
pub trait Trait: frame_system::Trait + pallet_timestamp::Trait {
	/// Mapping from address to account id.
	type AddressMapping: AddressMapping<Self::AccountId>;
	/// Currency type for withdraw and balance storage.
	type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
	/// Merge free balance from source to dest.
	type MergeAccount: MergeAccount<Self::AccountId>;
	/// Deposit for creating contract, would be reserved until contract deleted.
	type ContractExistentialDeposit: Get<BalanceOf<Self>>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// Precompiles associated with this EVM engine.
	type Precompiles: Precompiles;
	/// Chain ID of EVM.
	type ChainId: Get<u64>;
	/// EVM execution runner.
	type Runner: Runner<Self>;
	/// Convert gas to weight.
	type GasToWeight: Convert<u32, Weight>;

	/// EVM config used in the module.
	fn config() -> &'static Config {
		&ACALA_CONFIG
	}

	/// Required origin for creating system contract.
	type NetworkContractOrigin: EnsureOrigin<Self::Origin>;
	/// The EVM address for creating system contract.
	type NetworkContractSource: Get<H160>;
}

/// Storage key size and storage value size.
pub const STORAGE_SIZE: u32 = 64;

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct ContractInfo {
	pub storage_count: u32,
	pub code_hash: H256,
}

impl ContractInfo {
	pub fn total_storage_size(&self) -> u32 {
		self.storage_count.saturating_mul(STORAGE_SIZE)
	}
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct AccountInfo<Index> {
	pub nonce: Index,
	pub contract_info: Option<ContractInfo>,
}

impl<Index> AccountInfo<Index> {
	pub fn new(nonce: Index, contract_info: Option<ContractInfo>) -> Self {
		Self { nonce, contract_info }
	}
}

#[derive(Clone, Copy, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
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

decl_storage! {
	trait Store for Module<T: Trait> as EVM {
		Accounts get(fn accounts): map hasher(twox_64_concat) H160 => Option<AccountInfo<T::Index>>;
		AccountStorages get(fn account_storages):
			double_map hasher(twox_64_concat) H160, hasher(blake2_128_concat) H256 => H256;

		Codes get(fn codes): map hasher(identity) H256 => Vec<u8>;
		CodeInfos get(fn code_infos): map hasher(identity) H256 => Option<CodeInfo>;

		/// Next available system contract address.
		NetworkContractIndex get(fn network_contract_index) config(): u64;
	}

	add_extra_genesis {
		config(accounts): std::collections::BTreeMap<H160, GenesisAccount<BalanceOf<T>, T::Index>>;
		build(|config: &GenesisConfig<T>| {
			for (address, account) in &config.accounts {
				let account_id = T::AddressMapping::to_account(address);

				<Accounts<T>>::insert(address, <AccountInfo<T::Index>>::new(account.nonce, None));
				<Module<T>>::on_contract_initialization(address, account.code.clone(), Some(account.storage.len() as u32));

				T::Currency::deposit_creating(
					&account_id,
					account.balance,
				);

				for (index, value) in &account.storage {
					AccountStorages::insert(address, index, value);
				}
			}
		});
	}
}

decl_event! {
	/// EVM events
	pub enum Event<T> where
		<T as frame_system::Trait>::AccountId,
	{
		/// Ethereum events from contracts.
		Log(Log),
		/// A contract has been created at given \[address\].
		Created(H160),
		/// A contract was attempted to be created, but the execution failed. \[contract, exit_reason, output\]
		CreatedFailed(H160, ExitReason, Vec<u8>),
		/// A \[contract\] has been executed successfully with states applied.
		Executed(H160),
		/// A contract has been executed with errors. States are reverted with only gas fees applied. \[contract, exit_reason, output\]
		ExecutedFailed(H160, ExitReason, Vec<u8>),
		/// A deposit has been made at a given address. \[sender, address, value\]
		BalanceDeposit(AccountId, H160, U256),
		/// A withdrawal has been made from a given address. \[sender, address, value\]
		BalanceWithdraw(AccountId, H160, U256),
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Address not mapped
		AddressNotMapped,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Issue an EVM call operation. This is similar to a message call transaction in Ethereum.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		fn call(
			origin,
			target: H160,
			input: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::to_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			let info = T::Runner::call(source, target, input, value, gas_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Executed(target));
			} else {
				Module::<T>::deposit_event(Event::<T>::ExecutedFailed(target, info.exit_reason, info.output));
			}

			let used_gas: u32 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		/// Issue an EVM create operation. This is similar to a contract creation transaction in
		/// Ethereum.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		fn create(
			origin,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::to_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			let info = T::Runner::create(source, init, value, gas_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Module::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u32 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		/// Issue an EVM create2 operation.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		fn create2(
			origin,
			init: Vec<u8>,
			salt: H256,
			value: BalanceOf<T>,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::to_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			let info = T::Runner::create2(source, init, salt, value, gas_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Module::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u32 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		/// Issue an EVM create operation. The next available system contract address will be used as created contract address.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		fn create_network_contract(
			origin,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			let source = T::NetworkContractSource::get();
			let address = H160::from_low_u64_be(Self::network_contract_index());
			let info = T::Runner::create_at_address(source, init, value, address, gas_limit, T::config())?;

			NetworkContractIndex::mutate(|v| *v = v.saturating_add(One::one()));

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Module::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u32 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}
	}
}

impl<T: Trait> Module<T> {
	/// Remove an account.
	pub fn remove_account(address: &H160) {
		// Deref code, and remove it if ref count is zero.
		if let Some(AccountInfo {
			contract_info: Some(contract_info),
			..
		}) = Self::accounts(address)
		{
			CodeInfos::mutate_exists(&contract_info.code_hash, |maybe_code_info| {
				if let Some(code_info) = maybe_code_info.as_mut() {
					code_info.ref_count = code_info.ref_count.saturating_sub(1);
					if code_info.ref_count == 0 {
						Codes::remove(&contract_info.code_hash);
						*maybe_code_info = None;
					}
				}
			});
		}

		<Accounts<T>>::remove(address);
		AccountStorages::remove_prefix(address);
	}

	/// Get the account basic in EVM format.
	pub fn account_basic(address: &H160) -> Account {
		let account_id = T::AddressMapping::to_account(address);

		let nonce = Self::accounts(address).map_or(Default::default(), |account_info| account_info.nonce);
		let balance = T::Currency::free_balance(&account_id);

		Account {
			nonce: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(nonce)),
			balance: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(balance)),
		}
	}

	/// Get code hash at given address.
	pub fn code_hash_at_address(address: &H160) -> H256 {
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
	pub fn code_at_address(address: &H160) -> Vec<u8> {
		Self::codes(&Self::code_hash_at_address(address))
	}

	/// Handler on new contract initialization.
	///
	/// - Create new account for the contract.
	///   - For contracts initialized in genesis block, `storage_count` param
	///     needed to be provided.
	///   - For contracts initialized via dispatch calls, storage count would be
	///     read from initialized account storages.
	/// - Update codes info.
	/// - Save `code` if not saved yet.
	pub fn on_contract_initialization(address: &H160, code: Vec<u8>, storage_count: Option<u32>) {
		let code_hash = code_hash(&code.as_slice());
		let storage_count = storage_count.unwrap_or_else(|| AccountStorages::iter_prefix(address).count() as u32);
		let contract_info = ContractInfo {
			storage_count,
			code_hash,
		};
		Accounts::<T>::mutate(address, |maybe_account_info| {
			if let Some(account_info) = maybe_account_info.as_mut() {
				account_info.contract_info = Some(contract_info);
			} else {
				*maybe_account_info = Some(AccountInfo::<T::Index>::new(Default::default(), Some(contract_info)));
			}
		});

		CodeInfos::mutate_exists(&code_hash, |maybe_code_info| {
			if let Some(code_info) = maybe_code_info.as_mut() {
				code_info.ref_count = code_info.ref_count.saturating_add(1);
			} else {
				let new = CodeInfo {
					code_size: code.len() as u32,
					ref_count: 1,
				};
				*maybe_code_info = Some(new);

				Codes::insert(&code_hash, code);
			}
		});
	}

	/// Set account storage.
	pub fn set_storage(address: H160, index: H256, value: H256) {
		enum StorageChange {
			None,
			Added,
			Removed,
		}

		let mut storage_change = StorageChange::None;

		let default_value = H256::default();
		let is_prev_value_default = Self::account_storages(address, index) == default_value;

		if value == default_value {
			if !is_prev_value_default {
				storage_change = StorageChange::Removed;
			}

			AccountStorages::remove(address, index);
		} else {
			if is_prev_value_default {
				storage_change = StorageChange::Added;
			}

			AccountStorages::insert(address, index, value);
		}

		<Accounts<T>>::mutate(&address, |maybe_account_info| {
			if let Some(AccountInfo {
				contract_info: Some(contract_info),
				..
			}) = maybe_account_info.as_mut()
			{
				match storage_change {
					StorageChange::Added => contract_info.storage_count = contract_info.storage_count.saturating_add(1),
					StorageChange::Removed => {
						contract_info.storage_count = contract_info.storage_count.saturating_sub(1)
					}
					_ => (),
				}
			}
		});
	}
}

impl<T: Trait> EVMTrait for Module<T> {
	type Balance = BalanceOf<T>;

	fn execute(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u32,
		config: Option<evm::Config>,
	) -> Result<CallInfo, sp_runtime::DispatchError> {
		let info = T::Runner::call(
			source,
			target,
			input,
			value,
			gas_limit,
			config.as_ref().unwrap_or(T::config()),
		)?;

		if info.exit_reason.is_succeed() {
			Module::<T>::deposit_event(Event::<T>::Executed(target));
		} else {
			Module::<T>::deposit_event(Event::<T>::ExecutedFailed(
				target,
				info.exit_reason.clone(),
				info.output.clone(),
			));
		}

		Ok(info)
	}
}

pub struct CallKillAccount<T>(PhantomData<T>);
impl<T: Trait> OnKilledAccount<T::AccountId> for CallKillAccount<T> {
	fn on_killed_account(who: &T::AccountId) {
		if let Some(address) = T::AddressMapping::to_evm_address(who) {
			Module::<T>::remove_account(&address)
		}
	}
}

pub fn code_hash(code: &[u8]) -> H256 {
	H256::from_slice(Keccak256::digest(code).as_slice())
}
