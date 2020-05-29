#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_module, decl_storage,
	dispatch::Dispatchable,
	ensure,
	traits::{
		Currency, ExistenceRequirement, Get, Happened, LockIdentifier, OnKilledAccount, OnUnbalanced, StoredMap, Time,
		WithdrawReason, WithdrawReasons,
	},
	weights::{DispatchInfo, PostDispatchInfo},
	IsSubType,
};
use frame_system::{self as system, ensure_signed, AccountInfo};
use orml_traits::{MultiCurrency, MultiLockableCurrency, MultiReservableCurrency, OnReceived};
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{
		AccountIdConversion, DispatchInfoOf, SaturatedConversion, Saturating, SignedExtension, UniqueSaturatedInto,
		Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	ModuleId,
};
use sp_std::convert::{Infallible, TryFrom, TryInto};
use sp_std::prelude::*;
use support::DEXManager;

mod mock;
mod tests;

const ACCOUNTS_ID: LockIdentifier = *b"ACA/acct";

type MomentOf<T> = <<T as Trait>::Time as Time>::Moment;
type PalletBalanceOf<T> =
	<<T as pallet_transaction_payment::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait + pallet_transaction_payment::Trait + orml_currencies::Trait {
	type FreeTransferCount: Get<u8>;
	type FreeTransferPeriod: Get<MomentOf<Self>>;
	type FreeTransferDeposit: Get<Balance>;
	type AllCurrencyIds: Get<Vec<CurrencyId>>;
	type NativeCurrencyId: Get<CurrencyId>;
	type Time: Time;
	type Currency: MultiLockableCurrency<Self::AccountId, Moment = Self::BlockNumber, CurrencyId = CurrencyId, Balance = Balance>
		+ MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;
	type OnCreatedAccount: Happened<Self::AccountId>;
	type KillAccount: Happened<Self::AccountId>;
	type NewAccountDeposit: Get<Balance>;
	type TreasuryModuleId: Get<ModuleId>;
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		NotEnoughBalance,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Accounts {
		LastFreeTransfers get(fn last_free_transfers): map hasher(twox_64_concat) T::AccountId => Vec<MomentOf<T>>;
		FreeTransferEnabledAccounts get(fn free_transfer_enabled_accounts): map hasher(twox_64_concat) T::AccountId => Option<bool>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		const FreeTransferCount: u8 = T::FreeTransferCount::get();
		const FreeTransferPeriod: MomentOf<T> = T::FreeTransferPeriod::get();
		const FreeTransferDeposit: Balance = T::FreeTransferDeposit::get();
		const AllCurrencyIds: Vec<CurrencyId> = T::AllCurrencyIds::get();
		const NativeCurrencyId: CurrencyId = T::NativeCurrencyId::get();
		const NewAccountDeposit: Balance = T::NewAccountDeposit::get();
		const TreasuryModuleId: ModuleId = T::TreasuryModuleId::get();

		#[weight = 10_000]
		fn enable_free_transfer(origin) {
			let who = ensure_signed(origin)?;

			let native_currency_id = T::NativeCurrencyId::get();
			let free_transfer_deposit = T::FreeTransferDeposit::get();
			ensure!(<T as Trait>::Currency::free_balance(native_currency_id, &who) > free_transfer_deposit, Error::<T>::NotEnoughBalance);

			<T as Trait>::Currency::set_lock(ACCOUNTS_ID, native_currency_id, &who, T::FreeTransferDeposit::get());
			<FreeTransferEnabledAccounts<T>>::insert(who, true);
		}

		#[weight = 10_000]
		fn disable_free_transfers(origin) {
			let who = ensure_signed(origin)?;

			<T as Trait>::Currency::remove_lock(ACCOUNTS_ID, T::NativeCurrencyId::get(), &who);
			<FreeTransferEnabledAccounts<T>>::remove(who);
		}
	}
}

impl<T: Trait> Module<T>
where
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
	pub fn treasury_account_id() -> T::AccountId {
		T::TreasuryModuleId::get().into_account()
	}

	pub fn try_free_transfer(who: &T::AccountId) -> bool {
		let mut last_free_transfer = Self::last_free_transfers(who);
		let now = T::Time::now();
		let free_transfer_period = T::FreeTransferPeriod::get();

		// remove all the expired entries
		last_free_transfer.retain(|&x| x.saturating_add(free_transfer_period) > now);

		// check if can transfer for free
		if <FreeTransferEnabledAccounts<T>>::contains_key(who)
			&& last_free_transfer.len() < T::FreeTransferCount::get() as usize
		{
			// add entry to last_free_transfer
			last_free_transfer.push(now);
			<LastFreeTransfers<T>>::insert(who, last_free_transfer);
			true
		} else {
			false
		}
	}
}

/// Fork StoredMap in frame_system,  still use `Account` storage of frame_system.
impl<T: Trait> StoredMap<T::AccountId, T::AccountData> for Module<T> {
	fn get(k: &T::AccountId) -> T::AccountData {
		system::Account::<T>::get(k).data
	}

	fn is_explicit(k: &T::AccountId) -> bool {
		system::Account::<T>::contains_key(k)
	}

	fn insert(k: &T::AccountId, data: T::AccountData) {
		let existed = system::Account::<T>::contains_key(k);
		system::Account::<T>::mutate(k, |a| a.data = data);
		// if not existed before, create new account info
		if !existed {
			T::OnCreatedAccount::happened(&k);
		}
	}

	fn remove(k: &T::AccountId) {
		T::KillAccount::happened(k);
	}

	fn mutate<R>(k: &T::AccountId, f: impl FnOnce(&mut T::AccountData) -> R) -> R {
		let existed = system::Account::<T>::contains_key(k);
		let r = system::Account::<T>::mutate(k, |a| f(&mut a.data));
		if !existed {
			T::OnCreatedAccount::happened(&k);
		}
		r
	}

	fn mutate_exists<R>(k: &T::AccountId, f: impl FnOnce(&mut Option<T::AccountData>) -> R) -> R {
		Self::try_mutate_exists(k, |x| -> Result<R, Infallible> { Ok(f(x)) }).expect("Infallible; qed")
	}

	fn try_mutate_exists<R, E>(
		k: &T::AccountId,
		f: impl FnOnce(&mut Option<T::AccountData>) -> Result<R, E>,
	) -> Result<R, E> {
		system::Account::<T>::try_mutate_exists(k, |maybe_value| {
			let existed = maybe_value.is_some();
			let (maybe_prefix, mut maybe_data) = split_inner(maybe_value.take(), |account| {
				((account.nonce, account.refcount), account.data)
			});
			f(&mut maybe_data).map(|result| {
				// check and manipulate reserved account opening funds
				*maybe_value = maybe_data.map(|data| {
					let (nonce, refcount) = maybe_prefix.unwrap_or_default();
					AccountInfo { nonce, refcount, data }
				});
				(existed, maybe_value.is_some(), result)
			})
		})
		.map(|(existed, exists, v)| {
			if !existed && exists {
				T::OnCreatedAccount::happened(&k);
			} else if existed && !exists {
				//<system::Module<T>>::on_killed_account(k.clone());
				<T as system::Trait>::OnKilledAccount::on_killed_account(&k);
				// deposit event `KilledAccount` in system
			}
			v
		})
	}
}

/// Split an `option` into two constituent options, as defined by a `splitter` function.
pub fn split_inner<T, R, S>(option: Option<T>, splitter: impl FnOnce(T) -> (R, S)) -> (Option<R>, Option<S>) {
	match option {
		Some(inner) => {
			let (r, s) = splitter(inner);
			(Some(r), Some(s))
		}
		None => (None, None),
	}
}

impl<T: Trait> OnReceived<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn on_received(who: T::AccountId, currency_id: CurrencyId, amount: Balance) {}
}

impl<T: Trait> OnKilledAccount<T::AccountId> for Module<T> {
	fn on_killed_account(who: &T::AccountId) {
		<LastFreeTransfers<T>>::remove(who);
	}
}

/// Require the transactor pay for themselves and maybe include a tip to gain additional priority
/// in the queue.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T: Trait + Send + Sync>(#[codec(compact)] PalletBalanceOf<T>);

impl<T: Trait + Send + Sync> ChargeTransactionPayment<T> {
	/// utility constructor. Used only in client/factory code.
	pub fn from(fee: PalletBalanceOf<T>) -> Self {
		Self(fee)
	}
}

impl<T: Trait + Send + Sync> sp_std::fmt::Debug for ChargeTransactionPayment<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "ChargeTransactionPayment<{:?}>", self.0)
	}
	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Trait + Send + Sync> SignedExtension for ChargeTransactionPayment<T>
where
	PalletBalanceOf<T>: Send + Sync + TryFrom<Balance> + TryInto<Balance>,
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<orml_currencies::Module<T>, T>,
{
	const IDENTIFIER: &'static str = "AcalaChargeTransactionPayment";
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type Pre = ();

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> TransactionValidity {
		// pay any fees.
		let tip = self.0;

		// check call type
		let skip_pay_fee = match call.is_sub_type() {
			Some(orml_currencies::Call::transfer(..)) => <Module<T>>::try_free_transfer(who),
			_ => false,
		};

		let pay_fee = !skip_pay_fee;
		let pay_tip = !tip.is_zero();

		// skip payment withdraw if match conditions
		let fee = if pay_fee || pay_tip {
			let mut reason = WithdrawReasons::none();
			let fee = if pay_fee {
				reason.set(WithdrawReason::TransactionPayment);
				<pallet_transaction_payment::Module<T>>::compute_fee(len as u32, info, tip)
			} else {
				tip
			};
			if pay_tip {
				reason.set(WithdrawReason::Tip);
			}
			let imbalance = match <T as pallet_transaction_payment::Trait>::Currency::withdraw(
				who,
				fee,
				reason,
				ExistenceRequirement::KeepAlive,
			) {
				Ok(imbalance) => imbalance,
				Err(_) => return InvalidTransaction::Payment.into(),
			};
			<T as pallet_transaction_payment::Trait>::OnTransactionPayment::on_unbalanced(imbalance);
			fee
		} else {
			Zero::zero()
		};

		let mut r = ValidTransaction::default();
		// NOTE: we probably want to maximize the _fee (of any type) per weight unit_ here, which
		// will be a bit more than setting the priority to tip. For now, this is enough.
		r.priority = fee.saturated_into::<TransactionPriority>();
		Ok(r)
	}
}
