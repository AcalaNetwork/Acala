#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_module, decl_storage,
	dispatch::Dispatchable,
	traits::{
		Currency, ExistenceRequirement, Get, LockIdentifier, LockableCurrency, OnReapAccount, OnUnbalanced, Time,
		WithdrawReason, WithdrawReasons,
	},
	weights::DispatchInfo,
	IsSubType, Parameter,
};
use orml_traits::MultiCurrency;
use rstd::prelude::*;
use sp_runtime::{
	traits::{Bounded, SaturatedConversion, Saturating, SignedExtension, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
};
use system::ensure_signed;

mod mock;
mod tests;

const ACCOUNTS_ID: LockIdentifier = *b"ACA/acct";

type MomentOf<T> = <<T as Trait>::Time as Time>::Moment;
type PalletBalanceOf<T> =
	<<T as pallet_transaction_payment::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
type DepositBalanceOf<T> = <<T as Trait>::DepositCurrency as Currency<<T as system::Trait>::AccountId>>::Balance;
type DepositMomentOf<T> = <<T as Trait>::DepositCurrency as LockableCurrency<<T as system::Trait>::AccountId>>::Moment;

pub trait Trait: system::Trait + pallet_transaction_payment::Trait + orml_currencies::Trait {
	type FreeTransferCount: Get<u8>;
	type FreeTransferPeriod: Get<MomentOf<Self>>;
	type FreeTransferDeposit: Get<DepositBalanceOf<Self>>;
	type Time: Time;
	type Currency: MultiCurrency<Self::AccountId> + Send + Sync;
	type Call: Parameter
		+ Dispatchable<Origin = <Self as system::Trait>::Origin>
		+ IsSubType<orml_currencies::Module<Self>, Self>;
	type DepositCurrency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Accounts {
		LastFreeTransfers get(fn last_free_transfers): map hasher(blake2_256) T::AccountId => Vec<MomentOf<T>>;
		FreeTransferEnabledAccounts get(fn free_transfer_enabled_accounts): map hasher(blake2_256) T::AccountId => Option<()>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		const FreeTransferCount: u8 = T::FreeTransferCount::get();
		const FreeTransferPeriod: MomentOf<T> = T::FreeTransferPeriod::get();
		const FreeTransferDeposit: DepositBalanceOf<T> = T::FreeTransferDeposit::get();

		fn enable_free_transfer(origin) {
			let who = ensure_signed(origin)?;

			T::DepositCurrency::set_lock(ACCOUNTS_ID, &who, T::FreeTransferDeposit::get(), DepositMomentOf::<T>::max_value(), WithdrawReasons::all());
			<FreeTransferEnabledAccounts<T>>::insert(who, ());
		}

		fn disable_free_transfers(origin) {
			let who = ensure_signed(origin)?;

			T::DepositCurrency::remove_lock(ACCOUNTS_ID, &who);
			<FreeTransferEnabledAccounts<T>>::remove(who);
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn try_free_transfer(who: &T::AccountId) -> bool {
		let mut last_free_transfer = Self::last_free_transfers(who);
		let now = T::Time::now();
		let free_transfer_period = T::FreeTransferPeriod::get();

		// remove all the expired entries
		last_free_transfer.retain(|&x| x.saturating_add(free_transfer_period) > now);

		// check if can transfer for free
		if <FreeTransferEnabledAccounts<T>>::exists(who)
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

impl<T: Trait> OnReapAccount<T::AccountId> for Module<T> {
	fn on_reap_account(who: &T::AccountId) {
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

impl<T: Trait + Send + Sync> rstd::fmt::Debug for ChargeTransactionPayment<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut rstd::fmt::Formatter) -> rstd::fmt::Result {
		write!(f, "ChargeTransactionPayment<{:?}>", self.0)
	}
	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut rstd::fmt::Formatter) -> rstd::fmt::Result {
		Ok(())
	}
}

impl<T: Trait + Send + Sync> SignedExtension for ChargeTransactionPayment<T>
where
	PalletBalanceOf<T>: Send + Sync,
{
	const IDENTIFIER: &'static str = "ChargeTransactionPayment";
	type AccountId = T::AccountId;
	type Call = <T as Trait>::Call;
	type AdditionalSigned = ();
	type DispatchInfo = DispatchInfo;
	type Pre = ();
	fn additional_signed(&self) -> rstd::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: Self::DispatchInfo,
		len: usize,
	) -> TransactionValidity {
		// pay any fees.
		let tip = self.0;
		let fee: PalletBalanceOf<T> =
			pallet_transaction_payment::ChargeTransactionPayment::<T>::compute_fee(len as u32, info, tip);

		// check call type
		let call = match call.is_sub_type() {
			Some(call) => call,
			None => return Ok(ValidTransaction::default()),
		};
		let skip_pay_fee = match call {
			orml_currencies::Call::transfer(..) => <Module<T>>::try_free_transfer(who),
			_ => false,
		};

		// skip payment withdraw if match conditions
		if !skip_pay_fee {
			let imbalance = match <T as pallet_transaction_payment::Trait>::Currency::withdraw(
				who,
				fee,
				if tip.is_zero() {
					WithdrawReason::TransactionPayment.into()
				} else {
					WithdrawReason::TransactionPayment | WithdrawReason::Tip
				},
				ExistenceRequirement::KeepAlive,
			) {
				Ok(imbalance) => imbalance,
				Err(_) => return InvalidTransaction::Payment.into(),
			};
			<T as pallet_transaction_payment::Trait>::OnTransactionPayment::on_unbalanced(imbalance);
		}

		let mut r = ValidTransaction::default();
		// NOTE: we probably want to maximize the _fee (of any type) per weight unit_ here, which
		// will be a bit more than setting the priority to tip. For now, this is enough.
		r.priority = fee.saturated_into::<TransactionPriority>();
		Ok(r)
	}
}
