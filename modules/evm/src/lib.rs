#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]

pub mod precompiles;
pub mod runner;
mod tests;

pub use crate::precompiles::{Precompile, Precompiles};
pub use crate::runner::Runner;
pub use evm::{Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed};
pub use primitives::evm::{Account, CallInfo, CreateInfo, Log, Vicinity};

#[cfg(feature = "std")]
use codec::{Decode, Encode};
use evm::Config;
use frame_support::dispatch::DispatchResultWithPostInfo;
use frame_support::traits::{Currency, Get, ReservableCurrency};
use frame_support::weights::{Pays, PostDispatchInfo, Weight};
use frame_support::{decl_error, decl_event, decl_module, decl_storage};
use frame_system::ensure_signed;
use orml_traits::{account::MergeAccount, Happened};
use primitives::evm::AddressMapping;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{Convert, UniqueSaturatedInto};
use sp_std::{marker::PhantomData, vec::Vec};

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

/// Substrate system chain ID.
pub struct SystemChainId;

impl Get<u64> for SystemChainId {
	fn get() -> u64 {
		sp_io::misc::chain_id()
	}
}

static ISTANBUL_CONFIG: Config = Config::istanbul();

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
		&ISTANBUL_CONFIG
	}
}

#[cfg(feature = "std")]
#[derive(Clone, Eq, PartialEq, Encode, Decode, Debug, Serialize, Deserialize)]
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
		AccountNonces get(fn account_nonces): map hasher(twox_64_concat) H160 => T::Index;
		AccountCodes get(fn account_codes): map hasher(twox_64_concat) H160 => Vec<u8>;
		AccountStorages get(fn account_storages):
			double_map hasher(twox_64_concat) H160, hasher(blake2_128_concat) H256 => H256;
	}

	add_extra_genesis {
		config(accounts): std::collections::BTreeMap<H160, GenesisAccount<BalanceOf<T>, T::Index>>;
		build(|config: &GenesisConfig<T>| {
			for (address, account) in &config.accounts {
				let account_id = T::AddressMapping::to_account(address);

				AccountNonces::<T>::insert(&address, account.nonce);

				T::Currency::deposit_creating(
					&account_id,
					account.balance,
				);

				AccountCodes::insert(address, &account.code);

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

			let info = T::Runner::call(source, target, input, value, gas_limit)?;

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

			let info = T::Runner::create(source, init, value, gas_limit)?;

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

			let info = T::Runner::create2(source, init, salt, value, gas_limit)?;

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
		<AccountNonces<T>>::remove(address);
		AccountCodes::remove(address);
		AccountStorages::remove_prefix(address);
	}

	/// Get the account basic in EVM format.
	pub fn account_basic(address: &H160) -> Account {
		let account_id = T::AddressMapping::to_account(address);

		let nonce = Self::account_nonces(address);
		let balance = T::Currency::free_balance(&account_id);

		Account {
			nonce: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(nonce)),
			balance: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(balance)),
		}
	}
}

pub struct CallKillAccount<T>(PhantomData<T>);
impl<T: Trait> Happened<T::AccountId> for CallKillAccount<T> {
	fn happened(who: &T::AccountId) {
		if let Some(address) = T::AddressMapping::to_evm_address(who) {
			Module::<T>::remove_account(&address)
		}
	}
}
