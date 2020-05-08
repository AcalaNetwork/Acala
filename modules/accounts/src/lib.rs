#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_module, decl_storage,
	dispatch::Dispatchable,
	ensure,
	traits::{
		Currency, ExistenceRequirement, Get, LockIdentifier, LockableCurrency, OnKilledAccount, OnUnbalanced, Time,
		WithdrawReason, WithdrawReasons,
	},
	weights::{DispatchInfo, PostDispatchInfo},
	IsSubType,
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	traits::{DispatchInfoOf, SaturatedConversion, Saturating, SignedExtension, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
};
use sp_std::prelude::*;

mod mock;
mod tests;

const ACCOUNTS_ID: LockIdentifier = *b"ACA/acct";

type MomentOf<T> = <<T as Trait>::Time as Time>::Moment;
type PalletBalanceOf<T> =
	<<T as pallet_transaction_payment::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
type DepositBalanceOf<T> = <<T as Trait>::DepositCurrency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait + pallet_transaction_payment::Trait + orml_currencies::Trait {
	type FreeTransferCount: Get<u8>;
	type FreeTransferPeriod: Get<MomentOf<Self>>;
	type FreeTransferDeposit: Get<DepositBalanceOf<Self>>;
	type Time: Time;
	type DepositCurrency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
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
		const FreeTransferDeposit: DepositBalanceOf<T> = T::FreeTransferDeposit::get();

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		fn enable_free_transfer(origin) {
			let who = ensure_signed(origin)?;

			ensure!(T::DepositCurrency::free_balance(&who) > T::FreeTransferDeposit::get(), Error::<T>::NotEnoughBalance);

			T::DepositCurrency::set_lock(ACCOUNTS_ID, &who, T::FreeTransferDeposit::get(), WithdrawReasons::all());
			<FreeTransferEnabledAccounts<T>>::insert(who, true);
		}

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		fn disable_free_transfers(origin) {
			let who = ensure_signed(origin)?;

			T::DepositCurrency::remove_lock(ACCOUNTS_ID, &who);
			<FreeTransferEnabledAccounts<T>>::remove(who);
		}
	}
}

impl<T: Trait> Module<T>
where
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
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
	PalletBalanceOf<T>: Send + Sync,
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<orml_currencies::Module<T>, T>,
{
	const IDENTIFIER: &'static str = "ChargeTransactionPayment";
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
				pallet_transaction_payment::ChargeTransactionPayment::<T>::compute_fee(len as u32, info, tip)
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
