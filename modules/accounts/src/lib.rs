#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_module, decl_storage,
	dispatch::Dispatchable,
	traits::{Currency, ExistenceRequirement, Get, OnReapAccount, OnUnbalanced, Time, WithdrawReason},
	weights::DispatchInfo,
	IsSubType, Parameter,
};
use orml_traits::MultiCurrency;
use sp_runtime::{
	traits::{Convert, SaturatedConversion, Saturating, SignedExtension, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
};
//use support::{Ratio};
use rstd::prelude::*;

//type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type MomentOf<T> = <<T as Trait>::Time as Time>::Moment;
type PalletBalanceOf<T> =
	<<T as pallet_transaction_payment::Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait + pallet_transaction_payment::Trait + orml_currencies::Trait {
	type FreeTransferCount: Get<u8>;
	type FreeTransferPeriod: Get<MomentOf<Self>>;
	type Time: Time;
	type Currency: MultiCurrency<Self::AccountId> + Send + Sync;
	type Call: Parameter
		+ Dispatchable<Origin = <Self as system::Trait>::Origin>
		+ IsSubType<orml_currencies::Module<Self>, Self>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Accounts {
		LastFreeTransfers get(fn last_free_transfers): map T::AccountId => Vec<MomentOf<T>>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		const FreeTransferCount: u8 = T::FreeTransferCount::get();
		const FreeTransferPeriod: MomentOf<T> = T::FreeTransferPeriod::get();
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
		if last_free_transfer.len() < T::FreeTransferCount::get() as usize {
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

	/// Compute the final fee value for a particular transaction.
	///
	/// The final fee is composed of:
	///   - _base_fee_: This is the minimum amount a user pays for a transaction.
	///   - _len_fee_: This is the amount paid merely to pay for size of the transaction.
	///   - _weight_fee_: This amount is computed based on the weight of the transaction. Unlike
	///      size-fee, this is not input dependent and reflects the _complexity_ of the execution
	///      and the time it consumes.
	///   - _targeted_fee_adjustment_: This is a multiplier that can tune the final fee based on
	///     the congestion of the network.
	///   - (optional) _tip_: if included in the transaction, it will be added on top. Only signed
	///      transactions can have a tip.
	///
	/// final_fee = base_fee + targeted_fee_adjustment(len_fee + weight_fee) + tip;
	fn compute_fee(
		len: u32,
		info: <Self as SignedExtension>::DispatchInfo,
		tip: PalletBalanceOf<T>,
	) -> PalletBalanceOf<T>
	where
		PalletBalanceOf<T>: Sync + Send,
	{
		if info.pays_fee {
			let len = <PalletBalanceOf<T>>::from(len);
			let per_byte = <T as pallet_transaction_payment::Trait>::TransactionByteFee::get();
			let len_fee = per_byte.saturating_mul(len);

			let weight_fee = {
				// cap the weight to the maximum defined in runtime, otherwise it will be the `Bounded`
				// maximum of its data type, which is not desired.
				let capped_weight = info.weight.min(<T as system::Trait>::MaximumBlockWeight::get());
				<T as pallet_transaction_payment::Trait>::WeightToFee::convert(capped_weight)
			};

			// the adjustable part of the fee
			let adjustable_fee = len_fee.saturating_add(weight_fee);
			let targeted_fee_adjustment = <pallet_transaction_payment::Module<T>>::next_fee_multiplier();
			// adjusted_fee = adjustable_fee + (adjustable_fee * targeted_fee_adjustment)
			let adjusted_fee = targeted_fee_adjustment.saturated_multiply_accumulate(adjustable_fee);

			let base_fee = <T as pallet_transaction_payment::Trait>::TransactionBaseFee::get();
			let final_fee = base_fee.saturating_add(adjusted_fee).saturating_add(tip);

			final_fee
		} else {
			tip
		}
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
		let call = match call.is_sub_type() {
			Some(call) => call,
			None => return Ok(ValidTransaction::default()),
		};

		// check call type
		let skip_pay_fee = match call {
			orml_currencies::Call::transfer(..) => {
				// call try_free_transfer
				if <Module<T>>::try_free_transfer(who) {
					true
				} else {
					false
				}
			}
			_ => false,
		};

		// pay any fees.
		let tip = self.0;
		let fee: PalletBalanceOf<T> = Self::compute_fee(len as u32, info, tip);

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
