#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_module, decl_storage,
	dispatch::{DispatchResult, Dispatchable},
	ensure,
	traits::{
		Currency, ExistenceRequirement, Get, Happened, Imbalance, LockIdentifier, OnKilledAccount, OnUnbalanced,
		StoredMap, Time, WithdrawReason, WithdrawReasons,
	},
	weights::{DispatchInfo, PostDispatchInfo},
	IsSubType,
};
use frame_system::{self as system, ensure_signed, AccountInfo};
use orml_traits::{MultiCurrency, MultiLockableCurrency, MultiReservableCurrency, OnReceived};
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{
		AccountIdConversion, CheckedSub, DispatchInfoOf, PostDispatchInfoOf, SaturatedConversion, Saturating,
		SignedExtension, UniqueSaturatedInto, Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	FixedPointOperand, ModuleId,
};
use sp_std::convert::Infallible;
use sp_std::prelude::*;
use support::DEXManager;

mod mock;
mod tests;

const ACCOUNTS_ID: LockIdentifier = *b"ACA/acct";

type MomentOf<T> = <<T as Trait>::Time as Time>::Moment;
type PalletBalanceOf<T> =
	<<T as pallet_transaction_payment::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as pallet_transaction_payment::Trait>::Currency as Currency<
	<T as system::Trait>::AccountId,
>>::NegativeImbalance;

pub trait Trait: system::Trait + pallet_transaction_payment::Trait + orml_currencies::Trait {
	type FreeTransferCount: Get<u8>;
	type FreeTransferPeriod: Get<MomentOf<Self>>;
	type FreeTransferDeposit: Get<Balance>;
	type AllNonNativeCurrencyIds: Get<Vec<CurrencyId>>;
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
		NonZeroRefCount,
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
		const AllNonNativeCurrencyIds: Vec<CurrencyId> = T::AllNonNativeCurrencyIds::get();
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

		#[weight = 0]
		fn close_account(origin, to: Option<T::AccountId>) {
			let who = ensure_signed(origin)?;

			// check must allow death
			// if native/non-native currencies has locks, means ref_count shouldn't be zero, can not close the account
			ensure!(
				<system::Module<T>>::allow_death(&who),
				Error::<T>::NonZeroRefCount,
			);

			let native_currency_id = T::NativeCurrencyId::get();
			let treasury_account = Self::treasury_account_id();
			let specific_receiver = to.unwrap_or_else(|| treasury_account.clone());

			let total_reserved_native = <T as Trait>::Currency::reserved_balance(native_currency_id, &who);

			// unreserve all native currency
			<T as Trait>::Currency::unreserve(native_currency_id, &who, total_reserved_native);

			// reserved except for the NewAccountDeposit should be refund to `TreasuryModuleId`
			if let Some(refund_to_treasury_reserved) = total_reserved_native.checked_sub(T::NewAccountDeposit::get()) {
				// transfer refund to treasury seperately if specific_receiver is not treasury_account
				if treasury_account != specific_receiver {
					<T as Trait>::Currency::transfer(native_currency_id, &who, &treasury_account, refund_to_treasury_reserved)?;
				}
			}

			// transfer all free to specific_receiver
			<T as Trait>::Currency::transfer(native_currency_id, &who, &specific_receiver, <T as Trait>::Currency::free_balance(native_currency_id, &who))?;

			// handle other non-native currencies
			for currency_id in T::AllNonNativeCurrencyIds::get() {
				let reserved = <T as Trait>::Currency::reserved_balance(currency_id, &who);
				if !reserved.is_zero() {
					// unreserve all reserved
					<T as Trait>::Currency::unreserve(currency_id, &who, reserved);

					// transfer reserved amount to treasury_account seperately if specific_receiver is not treasury_account
					if treasury_account != specific_receiver {
						let _ = <T as Trait>::Currency::transfer(currency_id, &who, &treasury_account, reserved);
					}
				}

				// transfer all free to specific_receiver
				let _ = <T as Trait>::Currency::transfer(currency_id, &who, &specific_receiver, <T as Trait>::Currency::free_balance(currency_id, &who));
			}

			// finally kill the account
			T::KillAccount::happened(&who);
		}
	}
}

impl<T: Trait> Module<T> {
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

	/// Open account by reserve native token
	fn open_account(k: &T::AccountId) {
		let native_currency_id = T::NativeCurrencyId::get();
		if <T as Trait>::Currency::reserve(native_currency_id, k, T::NewAccountDeposit::get()).is_ok() {
			T::OnCreatedAccount::happened(&k);
		} else {
			let treasury_account = Self::treasury_account_id();

			// Note: will not reap treasury account even though it cannot reserve open account deposit
			// best practice is to ensure that the first transfer received by treasury account is sufficient to open an account.
			if *k != treasury_account {
				// send dust native currency to treasury account
				let _ = <T as Trait>::Currency::transfer(
					native_currency_id,
					k,
					&treasury_account,
					<T as Trait>::Currency::free_balance(native_currency_id, k),
				);

				// remove the account info pretend that opening account has never happened
				system::Account::<T>::remove(k);
			}
		}
	}
}

/// Note: Currently `pallet_balances` does not implement `OnReceived`,
/// which means here only do the preparations for opening an account by non-native currency,
/// acutal process of opening account is handled by `StoredMap`.
impl<T: Trait> OnReceived<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn on_received(who: &T::AccountId, currency_id: CurrencyId, _: Balance) {
		let native_currency_id = T::NativeCurrencyId::get();

		if !<Self as StoredMap<_, _>>::is_explicit(who) && currency_id != native_currency_id {
			let new_account_deposit = T::NewAccountDeposit::get();
			let supply_amount = T::DEX::get_supply_amount(currency_id, native_currency_id, new_account_deposit);

			// swap enough native currency to support subsequent opening account
			if <T as Trait>::Currency::free_balance(currency_id, who) >= supply_amount
				&& T::DEX::exchange_currency(
					who.clone(),
					currency_id,
					supply_amount,
					native_currency_id,
					new_account_deposit,
				)
				.is_ok()
			{
				// successful swap will cause changes in native currency,
				// which also means that it will open a new account
				return;
			}

			// open account will fail because there's no enough native token,
			// transfer all token as dust to treasury account.
			let treasury_account = Self::treasury_account_id();
			if who.clone() != treasury_account {
				let _ = <T as Trait>::Currency::transfer(
					currency_id,
					who,
					&treasury_account,
					<T as Trait>::Currency::free_balance(currency_id, who),
				);
			}
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
			Self::open_account(k);
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
				// Note: do not remove the AccountData storage even if the maybe_data is None
				let (nonce, refcount) = maybe_prefix.unwrap_or_default();
				let data = maybe_data.unwrap_or_default();
				*maybe_value = Some(AccountInfo { nonce, refcount, data });

				(existed, maybe_value.is_some(), result)
			})
		})
		.map(|(existed, exists, v)| {
			if !existed && exists {
				// need to open account
				Self::open_account(k);
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

impl<T: Trait> OnKilledAccount<T::AccountId> for Module<T> {
	fn on_killed_account(who: &T::AccountId) {
		<LastFreeTransfers<T>>::remove(who);
	}
}

/// Require the transactor pay for themselves and maybe include a tip to gain additional priority
/// in the queue.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T: Trait + Send + Sync>(#[codec(compact)] PalletBalanceOf<T>);

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

impl<T: Trait + Send + Sync> ChargeTransactionPayment<T>
where
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<orml_currencies::Module<T>, T>,
	PalletBalanceOf<T>: Send + Sync + FixedPointOperand,
{
	/// utility constructor. Used only in client/factory code.
	pub fn from(fee: PalletBalanceOf<T>) -> Self {
		Self(fee)
	}

	fn withdraw_fee(
		&self,
		who: &T::AccountId,
		call: &T::Call,
		info: &DispatchInfoOf<T::Call>,
		len: usize,
	) -> Result<(PalletBalanceOf<T>, Option<NegativeImbalanceOf<T>>), TransactionValidityError> {
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
		if pay_fee || pay_tip {
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

			// check native balance if is enough
			let native_is_enough = <T as pallet_transaction_payment::Trait>::Currency::free_balance(who)
				.checked_sub(&fee)
				.map_or(false, |new_free_balance| {
					<T as pallet_transaction_payment::Trait>::Currency::ensure_can_withdraw(
						who,
						fee,
						reason,
						new_free_balance,
					)
					.is_ok()
				});

			// try to use non-native currency to swap native currency by exchange with DEX
			if !native_is_enough {
				let native_currency_id = T::NativeCurrencyId::get();
				let other_currency_ids = T::AllNonNativeCurrencyIds::get();
				// Note: in fact, just obtain the gap between of fee and usable native currency amount,
				// but `Currency` does not expose interface to get usable balance by specific reason.
				// Here try to swap the whole fee by non-native currency.
				let balance_fee: Balance = fee.unique_saturated_into();

				// iterator non-native currencies to get enough fee
				for currency_id in other_currency_ids {
					let currency_amount = <T as Trait>::Currency::free_balance(currency_id, who);
					let supply_amount_needed = T::DEX::get_supply_amount(currency_id, native_currency_id, balance_fee);

					// TODO: consider slipperage
					if currency_amount >= supply_amount_needed
						&& T::DEX::exchange_currency(
							who.clone(),
							currency_id,
							supply_amount_needed,
							native_currency_id,
							balance_fee,
						)
						.is_ok()
					{
						// successfully swap, break iteration
						break;
					}
				}
			}

			// withdraw native currency as fee
			match <T as pallet_transaction_payment::Trait>::Currency::withdraw(
				who,
				fee,
				reason,
				ExistenceRequirement::KeepAlive,
			) {
				Ok(imbalance) => Ok((fee, Some(imbalance))),
				Err(_) => Err(InvalidTransaction::Payment.into()),
			}
		} else {
			Ok((Zero::zero(), None))
		}
	}
}

impl<T: Trait + Send + Sync> SignedExtension for ChargeTransactionPayment<T>
where
	PalletBalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand,
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<orml_currencies::Module<T>, T>,
{
	const IDENTIFIER: &'static str = "ChargeTransactionPayment";
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type Pre = (
		PalletBalanceOf<T>,
		Self::AccountId,
		Option<NegativeImbalanceOf<T>>,
		PalletBalanceOf<T>,
	);

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
		let (fee, _) = self.withdraw_fee(who, call, info, len)?;

		let mut r = ValidTransaction::default();
		// NOTE: we probably want to maximize the _fee (of any type) per weight unit_ here, which
		// will be a bit more than setting the priority to tip. For now, this is enough.
		r.priority = fee.saturated_into::<TransactionPriority>();
		Ok(r)
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		let (fee, imbalance) = self.withdraw_fee(who, call, info, len)?;
		Ok((self.0, who.clone(), imbalance, fee))
	}

	fn post_dispatch(
		pre: Self::Pre,
		info: &DispatchInfoOf<Self::Call>,
		post_info: &PostDispatchInfoOf<Self::Call>,
		len: usize,
		_result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		let (tip, who, imbalance, fee) = pre;
		if let Some(payed) = imbalance {
			let actual_fee =
				<pallet_transaction_payment::Module<T>>::compute_actual_fee(len as u32, info, post_info, tip);
			let refund = fee.saturating_sub(actual_fee);
			let actual_payment =
				match <T as pallet_transaction_payment::Trait>::Currency::deposit_into_existing(&who, refund) {
					Ok(refund_imbalance) => {
						// The refund cannot be larger than the up front payed max weight.
						// `PostDispatchInfo::calc_unspent` guards against such a case.
						match payed.offset(refund_imbalance) {
							Ok(actual_payment) => actual_payment,
							Err(_) => return Err(InvalidTransaction::Payment.into()),
						}
					}
					// We do not recreate the account using the refund. The up front payment
					// is gone in that case.
					Err(_) => payed,
				};
			let imbalances = actual_payment.split(tip);
			<T as pallet_transaction_payment::Trait>::OnTransactionPayment::on_unbalanceds(
				Some(imbalances.0).into_iter().chain(Some(imbalances.1)),
			);
		}
		Ok(())
	}
}
