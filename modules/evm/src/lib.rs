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
	traits::{BalanceStatus, Currency, EnsureOrigin, ExistenceRequirement, Get, OnKilledAccount, ReservableCurrency},
	transactional,
	weights::{Pays, PostDispatchInfo, Weight},
	RuntimeDebug,
};
use frame_system::{ensure_signed, EnsureOneOf, EnsureRoot, EnsureSigned};
use orml_traits::account::MergeAccount;
use primitives::evm::AddressMapping;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use sp_core::{H256, U256};
use sp_runtime::{
	traits::{CheckedAdd, CheckedSub, Convert, One, Saturating, UniqueSaturatedInto, Zero},
	Either,
};
use sp_std::{marker::PhantomData, vec::Vec};
use support::{EVMStateRentTrait, EVM as EVMTrait};

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub trait WeightInfo {
	fn add_storage_quota() -> Weight;
	fn remove_storage_quota() -> Weight;
	fn request_transfer_maintainer() -> Weight;
	fn cancel_transfer_maintainer() -> Weight;
	fn confirm_transfer_maintainer() -> Weight;
	fn reject_transfer_maintainer() -> Weight;
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
	/// Deposit for creating contract, would be reserved until contract deleted.
	type ContractExistentialDeposit: Get<BalanceOf<Self>>;
	/// Deposit for transferring the maintainer of the contract.
	type TransferMaintainerDeposit: Get<BalanceOf<Self>>;
	/// Storage required for per byte.
	type StorageDepositPerByte: Get<BalanceOf<Self>>;
	/// Storage quota default value.
	type StorageDefaultQuota: Get<u32>;
	/// Contract max code size.
	type MaxCodeSize: Get<u32>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// Precompiles associated with this EVM engine.
	type Precompiles: Precompiles;
	/// Chain ID of EVM.
	type ChainId: Get<u64>;

	/// Convert gas to weight.
	type GasToWeight: Convert<u32, Weight>;

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

/// Storage key size and storage value size.
pub const STORAGE_SIZE: u32 = 64;

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct ContractInfo<T: Config> {
	pub storage_count: u32,
	pub code_hash: H256,
	pub existential_deposit: BalanceOf<T>,
	pub maintainer: EvmAddress,
	pub deployed: bool,
}

impl<T: Config> ContractInfo<T> {
	pub fn total_storage_size(&self) -> u32 {
		self.storage_count.saturating_mul(STORAGE_SIZE)
	}
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct AccountInfo<T: Config> {
	pub nonce: T::Index,
	pub contract_info: Option<ContractInfo<T>>,
	pub storage_rent_deposit: BalanceOf<T>,
	pub storage_quota: u32,
	/// The storage_usage is the sum of additional storage required by all
	/// contracts.
	pub storage_usage: u32,
	pub developer_deposit: Option<BalanceOf<T>>,
}

impl<T: Config> AccountInfo<T> {
	pub fn new(nonce: T::Index) -> Self {
		Self {
			nonce,
			contract_info: None,
			storage_rent_deposit: Zero::zero(),
			storage_quota: T::StorageDefaultQuota::get(),
			storage_usage: Zero::zero(),
			developer_deposit: None,
		}
	}

	pub fn new_with_contract(nonce: T::Index, contract_info: ContractInfo<T>) -> Result<Self, DispatchError> {
		let storage_quota = T::StorageDefaultQuota::get();

		let code_size = CodeInfos::get(contract_info.code_hash).map_or(0, |code_info| code_info.code_size);
		let additional_storage = contract_info
			.total_storage_size()
			.saturating_add(code_size)
			.saturating_sub(storage_quota);

		if !additional_storage.is_zero() {
			// get maintainer quota and pay for the additional_storage
			Module::<T>::do_update_maintainer_storage_usage(&contract_info.maintainer, 0, additional_storage)?;
		}

		Ok(Self {
			nonce,
			contract_info: Some(contract_info),
			storage_rent_deposit: Zero::zero(),
			storage_quota,
			storage_usage: Zero::zero(),
			developer_deposit: None,
		})
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
		/// Pending transfer maintainers: double_map (contract, new_maintainer) => TransferMaintainerDeposit
		PendingTransferMaintainers get(fn pending_transfer_maintainers): double_map hasher(twox_64_concat) EvmAddress, hasher(twox_64_concat) EvmAddress => Option<BalanceOf<T>>;

		/// Next available system contract address.
		NetworkContractIndex get(fn network_contract_index) config(): u64;
	}

	add_extra_genesis {
		config(accounts): std::collections::BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>;
		build(|config: &GenesisConfig<T>| {
			for (address, account) in &config.accounts {
				let account_id = T::AddressMapping::get_account_id(address);

				let account_info = <AccountInfo<T>>::new(account.nonce);
				<Accounts<T>>::insert(address, account_info);

				T::Currency::deposit_creating(
					&account_id,
					account.balance,
				);

				if !account.code.is_empty() { // if code len > 0 then it's a contract
					<Module<T>>::on_contract_initialization(address, &EvmAddress::default(), account.code.clone(), Some(account.storage.len() as u32)).expect("Genesis contract shouldn't fail");

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
		/// Requested the transfer maintainer. \[contract, address\]
		RequestedTransferMaintainer(EvmAddress, EvmAddress),
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
		/// Storage quota not enough
		StorageQuotaNotEnough,
		/// Unreserve failed
		UnreserveFailed,
		/// Pending transfer maintainers exists
		PendingTransferMaintainersExists,
		/// Pending transfer maintainers not exists
		PendingTransferMaintainersNotExists,
		/// Contract development is not enabled
		ContractDevelopmentNotEnabled,
		/// Contract development is already enabled
		ContractDevelopmentAlreadyEnabled,
		/// Contract already deployed
		ContractAlreadyDeployed,
		/// Contract exceeds max code size
		ContractExceedsMaxCodeSize,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Deploy a contract need the existential deposit.
		const ContractExistentialDeposit: BalanceOf<T> = T::ContractExistentialDeposit::get();
		/// Deposit for transferring the maintainer of the contract.
		const TransferMaintainerDeposit: BalanceOf<T> = T::TransferMaintainerDeposit::get();
		/// Storage required for per byte.
		const StorageDepositPerByte: BalanceOf<T> = T::StorageDepositPerByte::get();
		/// Storage quota default value.
		const StorageDefaultQuota: u32 = T::StorageDefaultQuota::get();
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
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			let info = Runner::<T>::call(source, target, input, value, gas_limit, T::config())?;

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
		pub fn create(
			origin,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			let info = Runner::<T>::create(source, init, value, gas_limit, T::config())?;

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
		pub fn create2(
			origin,
			init: Vec<u8>,
			salt: H256,
			value: BalanceOf<T>,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let source = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			let info = Runner::<T>::create2(source, init, salt, value, gas_limit, T::config())?;

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
		pub fn create_network_contract(
			origin,
			init: Vec<u8>,
			value: BalanceOf<T>,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::NetworkContractOrigin::ensure_origin(origin)?;

			let source = T::NetworkContractSource::get();
			let address = EvmAddress::from_low_u64_be(Self::network_contract_index());
			let info = Runner::<T>::create_at_address(source, init, value, address, gas_limit, T::config())?;

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

		#[weight = <T as Config>::WeightInfo::add_storage_quota()]
		#[transactional]
		pub fn add_storage_quota(origin, contract: EvmAddress, bytes: u32) {
			let who = ensure_signed(origin)?;
			Self::do_add_storage_quota(who, contract, bytes)?;

			Module::<T>::deposit_event(Event::<T>::AddStorageQuota(contract, bytes));
		}

		#[weight = <T as Config>::WeightInfo::remove_storage_quota()]
		#[transactional]
		pub fn remove_storage_quota(origin, contract: EvmAddress, bytes: u32) {
			let who = ensure_signed(origin)?;
			Self::do_remove_storage_quota(who, contract, bytes)?;

			Module::<T>::deposit_event(Event::<T>::RemoveStorageQuota(contract, bytes));
		}

		#[weight = <T as Config>::WeightInfo::request_transfer_maintainer()]
		#[transactional]
		pub fn request_transfer_maintainer(origin, contract: EvmAddress) {
			let who = ensure_signed(origin)?;
			let new_maintainer = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			Self::do_request_transfer_maintainer(who, contract, new_maintainer)?;

			Module::<T>::deposit_event(Event::<T>::RequestedTransferMaintainer(contract, new_maintainer));
		}

		#[weight = <T as Config>::WeightInfo::cancel_transfer_maintainer()]
		#[transactional]
		pub fn cancel_transfer_maintainer(origin, contract: EvmAddress) {
			let who = ensure_signed(origin)?;
			let requester = T::AddressMapping::get_evm_address(&who).ok_or(Error::<T>::AddressNotMapped)?;

			Self::do_cancel_transfer_maintainer(who, contract, requester)?;

			Module::<T>::deposit_event(Event::<T>::CanceledTransferMaintainer(contract, requester));
		}

		#[weight = <T as Config>::WeightInfo::confirm_transfer_maintainer()]
		#[transactional]
		pub fn confirm_transfer_maintainer(origin, contract: EvmAddress, new_maintainer: EvmAddress) {
			let who = ensure_signed(origin)?;
			Self::do_confirm_transfer_maintainer(who, contract, new_maintainer)?;

			Module::<T>::deposit_event(Event::<T>::ConfirmedTransferMaintainer(contract, new_maintainer));
		}

		#[weight = <T as Config>::WeightInfo::reject_transfer_maintainer()]
		#[transactional]
		pub fn reject_transfer_maintainer(origin, contract: EvmAddress, invalid_maintainer: EvmAddress) {
			let who = ensure_signed(origin)?;
			Self::do_reject_transfer_maintainer(who, contract, invalid_maintainer)?;

			Module::<T>::deposit_event(Event::<T>::RejectedTransferMaintainer(contract, invalid_maintainer));
		}

		#[weight = <T as Config>::WeightInfo::deploy()]
		#[transactional]
		pub fn deploy(origin, contract: EvmAddress) {
			let who = ensure_signed(origin)?;
			let address = T::AddressMapping::get_or_create_evm_address(&who);
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
					let mut account_info = AccountInfo::<T>::new(Default::default());
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
	pub fn remove_account(address: &EvmAddress) -> Result<(), ExitError> {
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
	pub fn code_at_address(address: &EvmAddress) -> Vec<u8> {
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
	pub fn on_contract_initialization(
		address: &EvmAddress,
		maintainer: &EvmAddress,
		code: Vec<u8>,
		storage_count: Option<u32>,
	) -> Result<(), ExitError> {
		let code_hash = code_hash(&code.as_slice());
		let storage_count = storage_count.unwrap_or_else(|| AccountStorages::iter_prefix(address).count() as u32);
		let contract_info = ContractInfo {
			storage_count,
			code_hash,
			existential_deposit: T::ContractExistentialDeposit::get(),
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

		Accounts::<T>::mutate(address, |maybe_account_info| -> Result<(), ExitError> {
			if let Some(account_info) = maybe_account_info.as_mut() {
				let additional_storage = contract_info
					.total_storage_size()
					.saturating_add(code_size)
					.saturating_sub(account_info.storage_quota);
				if !additional_storage.is_zero() {
					// get maintainer quota and pay for the additional_storage
					Self::do_update_maintainer_storage_usage(&contract_info.maintainer, 0, additional_storage)
						.map_or_else(
							|_| Err(ExitError::Other("update maintainer storage usage failed".into())),
							|_| Ok(()),
						)?;
				}

				account_info.contract_info = Some(contract_info);
				Ok(())
			} else {
				let account_info = AccountInfo::<T>::new_with_contract(Default::default(), contract_info).map_or_else(
					|_| Err(ExitError::Other("update maintainer storage usage failed".into())),
					Ok,
				)?;
				*maybe_account_info = Some(account_info);
				Ok(())
			}
		})
	}

	/// Set account storage.
	pub fn set_storage(address: EvmAddress, index: H256, value: H256) -> Result<(), ExitError> {
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

		<Accounts<T>>::mutate(&address, |maybe_account_info| -> Result<(), ExitError> {
			if let Some(AccountInfo {
				contract_info: Some(contract_info),
				..
			}) = maybe_account_info.as_mut()
			{
				match storage_change {
					StorageChange::Added => {
						contract_info.storage_count = contract_info.storage_count.saturating_add(1);
					}
					StorageChange::Removed => {
						contract_info.storage_count = contract_info.storage_count.saturating_sub(1);
					}
					_ => (),
				}
			}
			Ok(())
		})
	}

	/// Get additional storage of the contract.
	fn additional_storage(contract: EvmAddress) -> u32 {
		Accounts::<T>::get(contract).map_or(0, |account_info| {
			let (total_storage_size, code_size) = account_info.contract_info.map_or((0, 0), |contract_info| {
				let code_size = CodeInfos::get(contract_info.code_hash).map_or(0, |code_info| code_info.code_size);
				(contract_info.total_storage_size(), code_size)
			});
			total_storage_size
				.saturating_add(code_size)
				.saturating_sub(account_info.storage_quota)
		})
	}

	fn do_add_storage_quota(who: T::AccountId, contract: EvmAddress, bytes: u32) -> DispatchResult {
		Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
			let account_info = maybe_account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info
				.contract_info
				.as_ref()
				.ok_or(Error::<T>::ContractNotFound)?;

			if bytes.is_zero() {
				return Ok(());
			}

			let adjust_deposit = T::StorageDepositPerByte::get().saturating_mul(bytes.into());
			let additional_storage = {
				let code_size = CodeInfos::get(contract_info.code_hash).map_or(0, |code_info| code_info.code_size);
				contract_info
					.total_storage_size()
					.saturating_add(code_size)
					.saturating_sub(account_info.storage_quota)
			};

			account_info.storage_rent_deposit = account_info
				.storage_rent_deposit
				.checked_add(&adjust_deposit)
				.ok_or(Error::<T>::NumOutOfBound)?;
			account_info.storage_quota = account_info
				.storage_quota
				.checked_add(bytes)
				.ok_or(Error::<T>::NumOutOfBound)?;

			let maintainer_account = T::AddressMapping::get_account_id(&contract_info.maintainer);
			if who != maintainer_account {
				T::Currency::transfer(
					&who,
					&maintainer_account,
					adjust_deposit,
					ExistenceRequirement::AllowDeath,
				)?;
			}
			T::Currency::reserve(&maintainer_account, adjust_deposit)?;

			if !additional_storage.is_zero() {
				if additional_storage > bytes {
					Self::do_update_maintainer_storage_usage(
						&contract_info.maintainer,
						additional_storage,
						additional_storage
							.checked_add(bytes)
							.expect("Non-negative integers sub can't overflow; qed"),
					)?;
				} else {
					Self::do_update_maintainer_storage_usage(&contract_info.maintainer, additional_storage, 0)?;
					account_info.storage_usage = Zero::zero();
				}
			}

			Ok(())
		})
	}

	fn do_remove_storage_quota(who: T::AccountId, contract: EvmAddress, bytes: u32) -> DispatchResult {
		Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
			let account_info = maybe_account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
			let contract_info = account_info
				.contract_info
				.as_ref()
				.ok_or(Error::<T>::ContractNotFound)?;

			let maintainer_account = T::AddressMapping::get_account_id(&contract_info.maintainer);
			ensure!(who == maintainer_account, Error::<T>::NoPermission);

			if bytes.is_zero() {
				return Ok(());
			}

			let adjust_deposit = T::StorageDepositPerByte::get().saturating_mul(bytes.into());
			ensure!(
				account_info.storage_rent_deposit >= adjust_deposit,
				Error::<T>::StorageQuotaNotEnough
			);

			account_info.storage_rent_deposit = account_info
				.storage_rent_deposit
				.checked_sub(&adjust_deposit)
				.ok_or(Error::<T>::NumOutOfBound)?;
			account_info.storage_quota = account_info
				.storage_quota
				.checked_sub(bytes)
				.ok_or(Error::<T>::NumOutOfBound)?;

			ensure!(
				account_info.storage_usage <= account_info.storage_quota,
				Error::<T>::StorageQuotaNotEnough
			);

			let additional_storage = {
				let code_size = CodeInfos::get(contract_info.code_hash).map_or(0, |code_info| code_info.code_size);
				contract_info
					.total_storage_size()
					.saturating_add(code_size)
					.saturating_sub(account_info.storage_quota)
			};

			ensure!(additional_storage.is_zero(), Error::<T>::StorageQuotaNotEnough);
			ensure!(
				T::Currency::unreserve(&who, adjust_deposit).is_zero(),
				Error::<T>::UnreserveFailed
			);

			Ok(())
		})
	}

	fn do_request_transfer_maintainer(
		who: T::AccountId,
		contract: EvmAddress,
		new_maintainer: EvmAddress,
	) -> DispatchResult {
		Accounts::<T>::get(contract).map_or(Err(Error::<T>::ContractNotFound), |account_info| {
			account_info
				.contract_info
				.map_or(Err(Error::<T>::ContractNotFound), |_| Ok(()))
		})?;
		ensure!(
			PendingTransferMaintainers::<T>::get(contract, new_maintainer).is_none(),
			Error::<T>::PendingTransferMaintainersExists
		);

		let transfer_maintainer_deposit = T::TransferMaintainerDeposit::get();
		T::Currency::reserve(&who, transfer_maintainer_deposit)?;
		PendingTransferMaintainers::<T>::insert(contract, new_maintainer, transfer_maintainer_deposit);
		Ok(())
	}

	fn do_cancel_transfer_maintainer(who: T::AccountId, contract: EvmAddress, requester: EvmAddress) -> DispatchResult {
		PendingTransferMaintainers::<T>::mutate_exists(
			contract,
			requester,
			|maybe_transfer_maintainer_deposit| -> DispatchResult {
				let transfer_maintainer_deposit = maybe_transfer_maintainer_deposit
					.take()
					.ok_or(Error::<T>::PendingTransferMaintainersNotExists)?;

				T::Currency::unreserve(&who, transfer_maintainer_deposit);
				Ok(())
			},
		)
	}

	fn do_confirm_transfer_maintainer(
		who: T::AccountId,
		contract: EvmAddress,
		new_maintainer: EvmAddress,
	) -> DispatchResult {
		PendingTransferMaintainers::<T>::mutate_exists(
			contract,
			new_maintainer,
			|maybe_transfer_maintainer_deposit| -> DispatchResult {
				let transfer_maintainer_deposit = maybe_transfer_maintainer_deposit
					.take()
					.ok_or(Error::<T>::PendingTransferMaintainersNotExists)?;

				Accounts::<T>::mutate(contract, |maybe_account_info| -> DispatchResult {
					let account_info = maybe_account_info.as_mut().ok_or(Error::<T>::ContractNotFound)?;
					let contract_info = account_info
						.contract_info
						.as_mut()
						.ok_or(Error::<T>::ContractNotFound)?;

					let maintainer_account = T::AddressMapping::get_account_id(&contract_info.maintainer);
					ensure!(who == maintainer_account, Error::<T>::NoPermission);

					let new_maintainer_account = T::AddressMapping::get_account_id(&new_maintainer);
					T::Currency::unreserve(&new_maintainer_account, transfer_maintainer_deposit);

					contract_info.maintainer = new_maintainer;
					Ok(())
				})?;

				Ok(())
			},
		)
	}

	fn do_reject_transfer_maintainer(
		who: T::AccountId,
		contract: EvmAddress,
		invalid_maintainer: EvmAddress,
	) -> DispatchResult {
		PendingTransferMaintainers::<T>::mutate_exists(
			contract,
			invalid_maintainer,
			|maybe_transfer_maintainer_deposit| -> DispatchResult {
				let transfer_maintainer_deposit = maybe_transfer_maintainer_deposit
					.take()
					.ok_or(Error::<T>::PendingTransferMaintainersNotExists)?;

				Accounts::<T>::get(contract).map_or(Err(Error::<T>::ContractNotFound), |account_info| {
					account_info
						.contract_info
						.map_or(Err(Error::<T>::ContractNotFound), |contract_info| {
							let maintainer_account = T::AddressMapping::get_account_id(&contract_info.maintainer);
							if who != maintainer_account {
								Err(Error::<T>::NoPermission)
							} else {
								Ok(())
							}
						})
				})?;

				// repatriate_reserved the reserve from requester to contract maintainer
				let from = T::AddressMapping::get_account_id(&invalid_maintainer);
				T::Currency::repatriate_reserved(&from, &who, transfer_maintainer_deposit, BalanceStatus::Free)?;

				Ok(())
			},
		)
	}

	fn do_update_maintainer_storage_usage(
		maintainer: &EvmAddress,
		pre_storage_usage: u32,
		current_storage_usage: u32,
	) -> DispatchResult {
		// get maintainer quota and pay for the additional_storage
		<Accounts<T>>::mutate(
			maintainer,
			|maybe_maintainer_account_info| -> Result<(), DispatchError> {
				if let Some(AccountInfo {
					storage_quota: maintainer_storage_quota,
					storage_usage: maintainer_storage_usage,
					..
				}) = maybe_maintainer_account_info.as_mut()
				{
					if let Some(delta) = current_storage_usage.checked_sub(pre_storage_usage) {
						*maintainer_storage_usage = maintainer_storage_usage
							.checked_add(delta)
							.ok_or(Error::<T>::NumOutOfBound)?;
					} else if let Some(delta) = pre_storage_usage.checked_sub(current_storage_usage) {
						*maintainer_storage_usage = maintainer_storage_usage
							.checked_sub(delta)
							.ok_or(Error::<T>::NumOutOfBound)?;
					}

					if *maintainer_storage_usage > *maintainer_storage_quota {
						return Err(Error::<T>::StorageQuotaNotEnough.into());
					}

					Ok(())
				} else {
					// maintainer not found.
					Err(Error::<T>::StorageQuotaNotEnough.into())
				}
			},
		)
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

			let pre_additional_storage = Self::additional_storage(contract);

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

			let additional_storage = contract_info
				.total_storage_size()
				.saturating_add(code_size)
				.saturating_sub(account_info.storage_quota);
			if additional_storage != pre_additional_storage {
				// get maintainer quota and pay for the additional_storage
				Self::do_update_maintainer_storage_usage(
					&contract_info.maintainer,
					pre_additional_storage,
					additional_storage,
				)?;
			}

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

			// delete contract & storage & refund to maintainer
			let additional_storage = Self::additional_storage(contract);
			if !additional_storage.is_zero() {
				// get maintainer quota and refund the additional_storage
				Self::do_update_maintainer_storage_usage(&contract_info.maintainer, additional_storage, 0)?;
			}

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
			// storage_rent_deposit + contract_info.existential_deposit + developer_deposit
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

impl<T: Config> EVMTrait for Module<T> {
	type Balance = BalanceOf<T>;

	fn execute(
		source: EvmAddress,
		target: EvmAddress,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u32,
		config: Option<evm::Config>,
	) -> Result<CallInfo, sp_runtime::DispatchError> {
		let info = Runner::<T>::call(
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

impl<T: Config> EVMStateRentTrait<T::AccountId, BalanceOf<T>> for Module<T> {
	fn query_contract_existential_deposit() -> BalanceOf<T> {
		T::ContractExistentialDeposit::get()
	}

	fn query_transfer_maintainer_deposit() -> BalanceOf<T> {
		T::TransferMaintainerDeposit::get()
	}

	fn query_qtorage_deposit_per_byte() -> BalanceOf<T> {
		T::StorageDepositPerByte::get()
	}

	fn query_storage_default_quota() -> u32 {
		T::StorageDefaultQuota::get()
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

	fn add_storage_quota(from: T::AccountId, contract: EvmAddress, bytes: u32) -> DispatchResult {
		Module::<T>::do_add_storage_quota(from, contract, bytes)
	}

	fn remove_storage_quota(from: T::AccountId, contract: EvmAddress, bytes: u32) -> DispatchResult {
		Module::<T>::do_remove_storage_quota(from, contract, bytes)
	}

	fn request_transfer_maintainer(from: T::AccountId, contract: EvmAddress) -> DispatchResult {
		let new_maintainer = T::AddressMapping::get_evm_address(&from).ok_or(Error::<T>::AddressNotMapped)?;
		Module::<T>::do_request_transfer_maintainer(from, contract, new_maintainer)
	}

	fn cancel_transfer_maintainer(from: T::AccountId, contract: EvmAddress) -> DispatchResult {
		let requester = T::AddressMapping::get_evm_address(&from).ok_or(Error::<T>::AddressNotMapped)?;
		Module::<T>::do_cancel_transfer_maintainer(from, contract, requester)
	}

	fn confirm_transfer_maintainer(
		from: T::AccountId,
		contract: EvmAddress,
		new_maintainer: EvmAddress,
	) -> DispatchResult {
		Module::<T>::do_confirm_transfer_maintainer(from, contract, new_maintainer)
	}
	fn reject_transfer_maintainer(
		from: T::AccountId,
		contract: EvmAddress,
		invalid_maintainer: EvmAddress,
	) -> DispatchResult {
		Module::<T>::do_reject_transfer_maintainer(from, contract, invalid_maintainer)
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
