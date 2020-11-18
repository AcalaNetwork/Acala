#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]

pub mod precompiles;
pub mod runner;
mod tests;

pub use crate::precompiles::{Precompile, Precompiles};
pub use crate::runner::Runner;
pub use evm::{Context, ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed};
pub use primitives::evm::{Account, CallInfo, CreateInfo, EnsureAddressOrigin, ExecutionInfo, Log, Vicinity};

#[cfg(feature = "std")]
use codec::{Decode, Encode};
use evm::Config;
use frame_support::dispatch::DispatchResultWithPostInfo;
use frame_support::traits::{Currency, Get};
use frame_support::weights::{Pays, Weight};
use frame_support::{decl_error, decl_event, decl_module, decl_storage};
use frame_system::RawOrigin;
use module_support::AccountMapping;
use orml_traits::{account::MergeAccount, Happened};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{Hasher, H160, H256, U256};
use sp_runtime::{traits::UniqueSaturatedInto, AccountId32};
use sp_std::{marker::PhantomData, vec::Vec};

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

/// Ensure that the origin is root.
pub struct EnsureAddressRoot<AccountId>(sp_std::marker::PhantomData<AccountId>);

impl<OuterOrigin, AccountId> EnsureAddressOrigin<OuterOrigin> for EnsureAddressRoot<AccountId>
where
	OuterOrigin: Into<Result<RawOrigin<AccountId>, OuterOrigin>> + From<RawOrigin<AccountId>>,
{
	type Success = ();

	fn try_address_origin(_address: &H160, origin: OuterOrigin) -> Result<(), OuterOrigin> {
		origin.into().and_then(|o| match o {
			RawOrigin::Root => Ok(()),
			r => Err(OuterOrigin::from(r)),
		})
	}
}

/// Ensure that the origin never happens.
pub struct EnsureAddressNever<AccountId>(sp_std::marker::PhantomData<AccountId>);

impl<OuterOrigin, AccountId> EnsureAddressOrigin<OuterOrigin> for EnsureAddressNever<AccountId> {
	type Success = AccountId;

	fn try_address_origin(_address: &H160, origin: OuterOrigin) -> Result<AccountId, OuterOrigin> {
		Err(origin)
	}
}

pub trait AddressMapping<A> {
	fn into_account_id(address: H160) -> A;
}

/// Identity address mapping.
pub struct IdentityAddressMapping;

impl AddressMapping<H160> for IdentityAddressMapping {
	fn into_account_id(address: H160) -> H160 {
		address
	}
}

/// Hashed address mapping.
pub struct HashedAddressMapping<H>(sp_std::marker::PhantomData<H>);

impl<H: Hasher<Out = H256>> AddressMapping<AccountId32> for HashedAddressMapping<H> {
	fn into_account_id(address: H160) -> AccountId32 {
		let mut data = [0u8; 24];
		data[0..4].copy_from_slice(b"evm:");
		data[4..24].copy_from_slice(&address[..]);
		let hash = H::hash(&data);

		AccountId32::from(Into::<[u8; 32]>::into(hash))
	}
}

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
	/// Allow the origin to call on behalf of given address.
	type CallOrigin: EnsureAddressOrigin<Self::Origin>;

	/// Mapping from address to account id.
	type AddressMapping: AddressMapping<Self::AccountId>;
	/// Mapping from account id to address.
	type AccountMapping: AccountMapping<Self::AccountId>;
	/// Currency type for withdraw and balance storage.
	type Currency: Currency<Self::AccountId>;
	/// Merge free balance from source to dest.
	type MergeAccount: MergeAccount<Self::AccountId>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// Precompiles associated with this EVM engine.
	type Precompiles: Precompiles;
	/// Chain ID of EVM.
	type ChainId: Get<u64>;
	/// EVM execution runner.
	type Runner: Runner<Self>;

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
		AccountNonces get(fn account_nonces): map hasher(blake2_128_concat) H160 => T::Index;
		AccountCodes get(fn account_codes): map hasher(blake2_128_concat) H160 => Vec<u8>;
		AccountStorages get(fn account_storages):
			double_map hasher(blake2_128_concat) H160, hasher(blake2_128_concat) H256 => H256;
	}

	add_extra_genesis {
		config(accounts): std::collections::BTreeMap<H160, GenesisAccount<BalanceOf<T>, T::Index>>;
		build(|config: &GenesisConfig<T>| {
			for (address, account) in &config.accounts {
				let account_id = T::AddressMapping::into_account_id(*address);

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
		/// A \[contract\] was attempted to be created, but the execution failed.
		CreatedFailed(H160),
		/// A \[contract\] has been executed successfully with states applied.
		Executed(H160),
		/// A \[contract\] has been executed with errors. States are reverted with only gas fees applied.
		ExecutedFailed(H160),
		/// A deposit has been made at a given address. \[sender, address, value\]
		BalanceDeposit(AccountId, H160, U256),
		/// A withdrawal has been made from a given address. \[sender, address, value\]
		BalanceWithdraw(AccountId, H160, U256),
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Not enough balance to perform action
		BalanceLow,
		/// Calculating total fee overflowed
		FeeOverflow,
		/// Calculating total payment overflowed
		PaymentOverflow,
		/// Withdraw fee failed
		WithdrawFailed,
		/// Gas price is too low.
		GasPriceTooLow,
		/// Nonce is invalid
		InvalidNonce,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Issue an EVM call operation. This is similar to a message call transaction in Ethereum.
		#[weight = *gas_limit as Weight]
		fn call(
			origin,
			source: H160,
			target: H160,
			input: Vec<u8>,
			value: U256,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::CallOrigin::ensure_address_origin(&source, origin)?;

			match T::Runner::call(
				source,
				target,
				input,
				value,
				gas_limit,
			)? {
				CallInfo {
					exit_reason: ExitReason::Succeed(_),
					..
				} => {
					Module::<T>::deposit_event(Event::<T>::Executed(target));
				},
				_ => {
					Module::<T>::deposit_event(Event::<T>::ExecutedFailed(target));
				},
			}

			Ok(Pays::No.into())
		}

		/// Issue an EVM create operation. This is similar to a contract creation transaction in
		/// Ethereum.
		#[weight = *gas_limit as Weight]
		fn create(
			origin,
			source: H160,
			init: Vec<u8>,
			value: U256,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::CallOrigin::ensure_address_origin(&source, origin)?;

			match T::Runner::create(
				source,
				init,
				value,
				gas_limit,
			)? {
				CreateInfo {
					exit_reason: ExitReason::Succeed(_),
					value: create_address,
					..
				} => {
					Module::<T>::deposit_event(Event::<T>::Created(create_address));
				},
				CreateInfo {
					exit_reason: _,
					value: create_address,
					..
				} => {
					Module::<T>::deposit_event(Event::<T>::CreatedFailed(create_address));
				},
			}

			Ok(Pays::No.into())
		}

		/// Issue an EVM create2 operation.
		#[weight = *gas_limit as Weight]
		fn create2(
			origin,
			source: H160,
			init: Vec<u8>,
			salt: H256,
			value: U256,
			gas_limit: u32,
		) -> DispatchResultWithPostInfo {
			T::CallOrigin::ensure_address_origin(&source, origin)?;

			match T::Runner::create2(
				source,
				init,
				salt,
				value,
				gas_limit,
			)? {
				CreateInfo {
					exit_reason: ExitReason::Succeed(_),
					value: create_address,
					..
				} => {
					Module::<T>::deposit_event(Event::<T>::Created(create_address));
				},
				CreateInfo {
					exit_reason: _,
					value: create_address,
					..
				} => {
					Module::<T>::deposit_event(Event::<T>::CreatedFailed(create_address));
				},
			}

			Ok(Pays::No.into())
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
		let account_id = T::AddressMapping::into_account_id(*address);

		let nonce = Self::account_nonces(&address);
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
		let address = T::AccountMapping::into_h160(who.clone());
		Module::<T>::remove_account(&address)
	}
}
