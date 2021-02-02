#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::or_fun_call)]

pub mod precompiles;
pub mod runner;

mod default_weight;
mod mock;
mod tests;

pub use crate::precompiles::{Precompile, Precompiles};
pub use crate::runner::Runner;
pub use evm::{Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed};
pub use primitives::evm::{Account, CallInfo, CreateInfo, EvmAddress, Log, Vicinity};

use codec::{Decode, Encode};
use evm::Config as EvmConfig;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo},
	ensure,
	error::BadOrigin,
	traits::{Currency, EnsureOrigin, ExistenceRequirement, Get, OnKilledAccount, ReservableCurrency},
	transactional,
	weights::{Pays, PostDispatchInfo, Weight},
	RuntimeDebug,
};
use frame_system::{ensure_root, ensure_signed, EnsureOneOf, EnsureRoot, EnsureSigned};
use orml_traits::account::MergeAccount;
use primitive_types::{H256, U256};
use primitives::evm::AddressMapping;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use sp_runtime::{
	traits::{Convert, DispatchInfoOf, One, PostDispatchInfoOf, SignedExtension, UniqueSaturatedInto},
	transaction_validity::TransactionValidityError,
	Either, TransactionOutcome,
};
use sp_std::{marker::PhantomData, vec::Vec};
use support::{EVMStateRentTrait, ExecutionMode, InvokeContext, EVM as EVMTrait};

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub trait WeightInfo {
	fn transfer_maintainer() -> Weight;
	fn deploy() -> Weight;
	fn deploy_free() -> Weight;
	fn enable_contract_development() -> Weight;
	fn disable_contract_development() -> Weight;
	fn set_code() -> Weight;
	fn selfdestruct() -> Weight;
}

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

/// EVM module trait
pub trait Config: frame_system::Config + pallet_timestamp::Config {
	/// Mapping from address to account id.
	type AddressMapping: AddressMapping<Self::AccountId>;
	/// Currency type for withdraw and balance storage.
	type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
	/// Merge free balance from source to dest.
	type MergeAccount: MergeAccount<Self::AccountId>;
	/// Charge extra bytes for creating a contract, would be reserved until the
	/// contract deleted.
	type NewContractExtraBytes: Get<u32>;
	/// Storage required for per byte.
	type StorageDepositPerByte: Get<BalanceOf<Self>>;
	/// Contract max code size.
	type MaxCodeSize: Get<u32>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// Precompiles associated with this EVM engine.
	type Precompiles: Precompiles;
	/// Chain ID of EVM.
	type ChainId: Get<u64>;

	/// Convert gas to weight.
	type GasToWeight: Convert<u64, Weight>;

	/// EVM config used in the module.
	fn config() -> &'static EvmConfig {
		&ACALA_CONFIG
	}

	/// Required origin for creating system contract.
	type NetworkContractOrigin: EnsureOrigin<Self::Origin>;
	/// The EVM address for creating system contract.
	type NetworkContractSource: Get<EvmAddress>;

	/// Deposit for the developer.
	type DeveloperDeposit: Get<BalanceOf<Self>>;
	/// The fee for deploying the contract.
	type DeploymentFee: Get<BalanceOf<Self>>;
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
	pub developer_deposit: Option<BalanceOf<T>>,
}

impl<T: Config> AccountInfo<T> {
	pub fn new(nonce: T::Index, contract_info: Option<ContractInfo>) -> Self {
		Self {
			nonce,
			contract_info,
			developer_deposit: None,
		}
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
	trait Store for Module<T: Config> as EVM {
		Accounts get(fn accounts): map hasher(twox_64_concat) EvmAddress => Option<AccountInfo<T>>;
		AccountStorages get(fn account_storages):
			double_map hasher(twox_64_concat) EvmAddress, hasher(blake2_128_concat) H256 => H256;

		Codes get(fn codes): map hasher(identity) H256 => Vec<u8>;
		CodeInfos get(fn code_infos): map hasher(identity) H256 => Option<CodeInfo>;

		/// Next available system contract address.
		NetworkContractIndex get(fn network_contract_index) config(): u64;

		/// Extrinsics origin for the current tx.
		ExtrinsicOrigin get(fn extrinsic_origin): Option<T::AccountId>;
	}

	add_extra_genesis {
		config(accounts): std::collections::BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>;
		build(|config: &GenesisConfig<T>| {
			for (address, account) in &config.accounts {
				let account_id = T::AddressMapping::get_account_id(address);

				let account_info = <AccountInfo<T>>::new(account.nonce, None);
				<Accounts<T>>::insert(address, account_info);

				T::Currency::deposit_creating(
					&account_id,
					account.balance,
				);

				if !account.code.is_empty() { // if code len > 0 then it's a contract
					<Module<T>>::on_contract_initialization(address, &EvmAddress::default(), account.code.clone()).expect("Genesis contract shouldn't fail");

					#[cfg(not(feature = "with-ethereum-compatibility"))]
					<Module<T>>::mark_deployed(*address, None).expect("Genesis contract shouldn't fail");

					for (index, value) in &account.storage {
						AccountStorages::insert(address, index, value);
					}
				}
			}
		});
	}
}

decl_event! {
	/// EVM events
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
	{
		/// Ethereum events from contracts.
		Log(Log),
		/// A contract has been created at given \[address\].
		Created(EvmAddress),
		/// A contract was attempted to be created, but the execution failed. \[contract, exit_reason, output\]
		CreatedFailed(EvmAddress, ExitReason, Vec<u8>),
		/// A \[contract\] has been executed successfully with states applied.
		Executed(EvmAddress),
		/// A contract has been executed with errors. States are reverted with only gas fees applied. \[contract, exit_reason, output\]
		ExecutedFailed(EvmAddress, ExitReason, Vec<u8>),
		/// A deposit has been made at a given address. \[sender, address, value\]
		BalanceDeposit(AccountId, EvmAddress, U256),
		/// A withdrawal has been made from a given address. \[sender, address, value\]
		BalanceWithdraw(AccountId, EvmAddress, U256),
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
		ContractDevelopmentEnabled(AccountId),
		/// Disabled contract development. \[who\]
		ContractDevelopmentDisabled(AccountId),
		/// Deployed contract. \[contract\]
		ContractDeployed(EvmAddress),
		/// Set contract code. \[contract\]
		ContractSetCode(EvmAddress),
		/// Selfdestructed contract code. \[contract\]
		ContractSelfdestructed(EvmAddress),
	}
}

decl_error! {
	pub enum Error for Module<T: Config> {
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
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Deploy a contract need the extra bytes.
		const NewContractExtraBytes: u32 = T::NewContractExtraBytes::get();
		/// Storage required for per byte.
		const StorageDepositPerByte: BalanceOf<T> = T::StorageDepositPerByte::get();
		/// Contract max code size.
		const MaxCodeSize: u32 = T::MaxCodeSize::get();
		/// Deposit for the developer.
		const DeveloperDeposit: BalanceOf<T> = T::DeveloperDeposit::get();
		/// The fee for deploying the contract.
		const DeploymentFee: BalanceOf<T> = T::DeploymentFee::get();

		/// Issue an EVM call operation. This is similar to a message call transaction in Ethereum.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		pub fn call(
			origin,
			target: EvmAddress,
			input: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = Runner::<T>::call(source, source, target, input, value, gas_limit, storage_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Executed(target));
			} else {
				Module::<T>::deposit_event(Event::<T>::ExecutedFailed(target, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		#[weight = T::GasToWeight::convert(*gas_limit)]
		#[transactional]
		pub fn scheduled_call(
			origin,
			from: EvmAddress,
			target: EvmAddress,
			input: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			// let from_account = T::AddressMapping::get_account_id(&from);
			// unreserve the deposit for gas_limit and storage_limit
			// TODO: https://github.com/AcalaNetwork/Acala/issues/700
			// let total_fee = T::StorageDepositPerByte::get().saturating_mul(storage_limit.into()).saturating_add(gas_limit.into());
			// T::Currency::unreserve(&from_account, total_fee);

			let info = Runner::<T>::call(from, from, target, input, value, gas_limit, storage_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Executed(target));
			} else {
				Module::<T>::deposit_event(Event::<T>::ExecutedFailed(target, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		/// Issue an EVM create operation. This is similar to a contract creation transaction in
		/// Ethereum.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		pub fn create(
			origin,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_or_create_evm_address(&who);

			let info = Runner::<T>::create(source, init, value, gas_limit, storage_limit, T::config())?;

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Module::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		/// Issue an EVM create2 operation.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		pub fn create2(
			origin,
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
				Module::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Module::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		/// Issue an EVM create operation. The next available system contract address will be used as created contract address.
		#[weight = T::GasToWeight::convert(*gas_limit)]
		pub fn create_network_contract(
			origin,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u64,
			storage_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			let source = T::NetworkContractSource::get();
			let address = EvmAddress::from_low_u64_be(Self::network_contract_index());
			let info = Runner::<T>::create_at_address(source, init, value, address, gas_limit, storage_limit, T::config())?;

			NetworkContractIndex::mutate(|v| *v = v.saturating_add(One::one()));

			if info.exit_reason.is_succeed() {
				Module::<T>::deposit_event(Event::<T>::Created(info.address));
			} else {
				Module::<T>::deposit_event(Event::<T>::CreatedFailed(info.address, info.exit_reason, info.output));
			}

			let used_gas: u64 = info.used_gas.unique_saturated_into();

			Ok(PostDispatchInfo {
				actual_weight: Some(T::GasToWeight::convert(used_gas)),
				pays_fee: Pays::Yes
			})
		}

		#[weight = <T as Config>::WeightInfo::transfer_maintainer()]
		#[transactional]
		pub fn transfer_maintainer(origin, contract: EvmAddress, new_maintainer: EvmAddress) {
			let who = ensure_signed(origin)?;
			Self::do_transfer_maintainer(who, contract, new_maintainer)?;

			Module::<T>::deposit_event(Event::<T>::TransferredMaintainer(contract, new_maintainer));
		}

		#[weight = <T as Config>::WeightInfo::deploy()]
		#[transactional]
		pub fn deploy(origin, contract: EvmAddress) {
			let who = ensure_signed(origin)?;
			let address = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
			T::Currency::transfer(&who, &T::TreasuryAccount::get(), T::DeploymentFee::get(), ExistenceRequirement::AllowDeath)?;
			Self::mark_deployed(contract, Some(address))?;
			Module::<T>::deposit_event(Event::<T>::ContractDeployed(contract));
		}

		#[weight = <T as Config>::WeightInfo::deploy_free()]
		#[transactional]
		pub fn deploy_free(origin, contract: EvmAddress) {
			T::FreeDeploymentOrigin::ensure_origin(origin)?;
			Self::mark_deployed(contract, None)?;
			Module::<T>::deposit_event(Event::<T>::ContractDeployed(contract));
		}

		#[weight = <T as Config>::WeightInfo::enable_contract_development()]
		#[transactional]
		pub fn enable_contract_development(origin) {
			let who = ensure_signed(origin)?;
			let address = T::AddressMapping::get_or_create_evm_address(&who);
			T::Currency::reserve(&who, T::DeveloperDeposit::get())?;
			Accounts::<T>::mutate(address, |maybe_account_info| -> DispatchResult {
				if let Some(account_info) = maybe_account_info.as_mut() {
					ensure!(account_info.developer_deposit.is_none(), Error::<T>::ContractDevelopmentAlreadyEnabled);
					account_info.developer_deposit = Some(T::DeveloperDeposit::get());
				} else {
					let mut account_info = AccountInfo::<T>::new(Default::default(), None);
					account_info.developer_deposit = Some(T::DeveloperDeposit::get());
					*maybe_account_info = Some(account_info);
				}
				Ok(())
			})?;
			Module::<T>::deposit_event(Event::<T>::ContractDevelopmentEnabled(who));
		}

		#[weight = <T as Config>::WeightInfo::disable_contract_development()]
		#[transactional]
		pub fn disable_contract_development(origin) {
			let who = ensure_signed(origin)?;
			let address = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
			let deposit = Accounts::<T>::mutate(address, |maybe_account_info| -> Result<BalanceOf<T>, Error<T>> {
				let account_info = maybe_account_info.as_mut().ok_or(Error::<T>::ContractDevelopmentNotEnabled)?;
				account_info.developer_deposit.take().ok_or(Error::<T>::ContractDevelopmentNotEnabled)
			})?;
			T::Currency::unreserve(&who, deposit);
			Module::<T>::deposit_event(Event::<T>::ContractDevelopmentDisabled(who));
		}

		#[weight = <T as Config>::WeightInfo::set_code()]
		#[transactional]
		pub fn set_code(origin, contract: EvmAddress, code: Vec<u8>) {
			let root_or_signed = Self::ensure_root_or_signed(origin)?;
			Self::do_set_code(root_or_signed, contract, code)?;

			Module::<T>::deposit_event(Event::<T>::ContractSetCode(contract));
		}

		#[weight = <T as Config>::WeightInfo::selfdestruct()]
		#[transactional]
		pub fn selfdestruct(origin, contract: EvmAddress) {
			let who = ensure_signed(origin)?;
			let maintainer = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;
			Self::do_selfdestruct(who, &maintainer, contract)?;

			Module::<T>::deposit_event(Event::<T>::ContractSelfdestructed(contract));
		}
	}
}

impl<T: Config> Module<T> {
	/// Remove an account.
	pub fn remove_account(address: &EvmAddress) -> Result<u32, ExitError> {
		let mut size = 0u32;

		// Deref code, and remove it if ref count is zero.
		if let Some(AccountInfo {
			contract_info: Some(contract_info),
			..
		}) = Self::accounts(address)
		{
			CodeInfos::mutate_exists(&contract_info.code_hash, |maybe_code_info| {
				if let Some(code_info) = maybe_code_info.as_mut() {
					size = code_info.code_size;
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

		Ok(size)
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
	pub fn code_at_address(address: &EvmAddress) -> Vec<u8> {
		Self::codes(&Self::code_hash_at_address(address))
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
		let code_hash = code_hash(&code.as_slice());
		let contract_info = ContractInfo {
			code_hash,
			maintainer: *maintainer,
			#[cfg(feature = "with-ethereum-compatibility")]
			deployed: true,
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			deployed: false,
		};

		let code_size = code.len() as u32;
		if code_size > T::MaxCodeSize::get() {
			return Err(ExitError::OutOfGas);
		}
		CodeInfos::mutate_exists(&code_hash, |maybe_code_info| {
			if let Some(code_info) = maybe_code_info.as_mut() {
				code_info.ref_count = code_info.ref_count.saturating_add(1);
			} else {
				let new = CodeInfo {
					code_size,
					ref_count: 1,
				};
				*maybe_code_info = Some(new);

				Codes::insert(&code_hash, code);
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

		Ok(())
	}

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

	fn do_set_code(root_or_signed: Either<(), T::AccountId>, contract: EvmAddress, code: Vec<u8>) -> DispatchResult {
		Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
			let account_info = maybe_account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info
				.contract_info
				.as_ref()
				.ok_or(Error::<T>::ContractNotFound)?;

			if let Either::Right(signer) = root_or_signed {
				let maintainer = T::AddressMapping::get_evm_address(&signer).ok_or(Error::<T>::AddressNotMapped)?;
				ensure!(contract_info.maintainer == maintainer, Error::<T>::NoPermission);
				ensure!(!contract_info.deployed, Error::<T>::ContractAlreadyDeployed);
			}

			let code_size = code.len() as u32;
			let code_hash = code_hash(&code.as_slice());
			if code_hash == contract_info.code_hash {
				return Ok(());
			}

			ensure!(
				code_size <= T::MaxCodeSize::get(),
				Error::<T>::ContractExceedsMaxCodeSize
			);

			CodeInfos::mutate_exists(&code_hash, |maybe_code_info| {
				if let Some(code_info) = maybe_code_info.as_mut() {
					code_info.ref_count = code_info.ref_count.saturating_add(1);
				} else {
					let new = CodeInfo {
						code_size,
						ref_count: 1,
					};
					*maybe_code_info = Some(new);

					Codes::insert(&code_hash, code);
				}
			});

			Ok(())
		})?;

		Ok(())
	}

	fn do_selfdestruct(who: T::AccountId, maintainer: &EvmAddress, contract: EvmAddress) -> DispatchResult {
		Accounts::<T>::mutate_exists(contract, |maybe_account_info| -> DispatchResult {
			let account_info = maybe_account_info.take().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info
				.contract_info
				.as_ref()
				.ok_or(Error::<T>::ContractNotFound)?;

			ensure!(contract_info.maintainer == *maintainer, Error::<T>::NoPermission);
			ensure!(!contract_info.deployed, Error::<T>::ContractAlreadyDeployed);

			AccountStorages::remove_prefix(contract);

			CodeInfos::mutate_exists(&contract_info.code_hash, |maybe_code_info| {
				if let Some(code_info) = maybe_code_info.as_mut() {
					code_info.ref_count = code_info.ref_count.saturating_sub(1);
					if code_info.ref_count == 0 {
						Codes::remove(&contract_info.code_hash);
						*maybe_code_info = None;
					}
				}
			});

			let contract_account_id = T::AddressMapping::get_account_id(&contract);
			T::Currency::unreserve(
				&contract_account_id,
				T::Currency::reserved_balance(&contract_account_id),
			);
			T::MergeAccount::merge_account(&contract_account_id, &who)?;

			Ok(())
		})?;

		Ok(())
	}

	fn ensure_root_or_signed(o: T::Origin) -> Result<Either<(), T::AccountId>, BadOrigin> {
		EnsureOneOf::<T::AccountId, EnsureRoot<T::AccountId>, EnsureSigned<T::AccountId>>::try_origin(o)
			.map_or(Err(BadOrigin), Ok)
	}
}

impl<T: Config> EVMTrait<T::AccountId> for Module<T> {
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
							Module::<T>::deposit_event(Event::<T>::Executed(context.contract));
							TransactionOutcome::Commit(Ok(info))
						} else {
							Module::<T>::deposit_event(Event::<T>::ExecutedFailed(
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

impl<T: Config> EVMStateRentTrait<T::AccountId, BalanceOf<T>> for Module<T> {
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
		Module::<T>::do_transfer_maintainer(from, contract, new_maintainer)
	}
}

pub struct CallKillAccount<T>(PhantomData<T>);
impl<T: Config> OnKilledAccount<T::AccountId> for CallKillAccount<T> {
	fn on_killed_account(who: &T::AccountId) {
		if let Some(address) = T::AddressMapping::get_evm_address(who) {
			let _ = Module::<T>::remove_account(&address);
		}
		let address = T::AddressMapping::get_default_evm_address(who);
		let _ = Module::<T>::remove_account(&address);
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
