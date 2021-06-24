// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # Transaction Payment Module
//!
//! ## Overview
//!
//! Transaction payment module is responsible for charge fee and tip in
//! different currencies

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	dispatch::{DispatchResult, Dispatchable},
	pallet_prelude::*,
	traits::{
		Currency, ExistenceRequirement, Imbalance, NamedReservableCurrency, OnUnbalanced, SameOrOther, WithdrawReasons,
	},
	weights::{DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo, WeightToFeePolynomial},
};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, InclusionFee};
use primitives::{Balance, CurrencyId, ReserveIdentifier};
use sp_runtime::{
	traits::{
		CheckedSub, Convert, DispatchInfoOf, PostDispatchInfoOf, SaturatedConversion, Saturating, SignedExtension,
		UniqueSaturatedInto, Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	FixedPointNumber, FixedPointOperand, FixedU128, Perquintill,
};
use sp_std::{prelude::*, vec};
use support::{DEXManager, Ratio, TransactionPayment};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

/// Fee multiplier.
pub type Multiplier = FixedU128;

type PalletBalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// A struct to update the weight multiplier per block. It implements
/// `Convert<Multiplier, Multiplier>`, meaning that it can convert the
/// previous multiplier to the next one. This should be called on
/// `on_finalize` of a block, prior to potentially cleaning the weight data
/// from the system module.
///
/// given:
///     s = previous block weight
///     s'= ideal block weight
///     m = maximum block weight
///     diff = (s - s')/m
///     v = 0.00001
///     t1 = (v * diff)
///     t2 = (v * diff)^2 / 2
/// then:
///     next_multiplier = prev_multiplier * (1 + t1 + t2)
///
/// Where `(s', v)` must be given as the `Get` implementation of the `T`
/// generic type. Moreover, `M` must provide the minimum allowed value for
/// the multiplier. Note that a runtime should ensure with tests that the
/// combination of this `M` and `V` is not such that the multiplier can drop
/// to zero and never recover.
///
/// note that `s'` is interpreted as a portion in the _normal transaction_
/// capacity of the block. For example, given `s' == 0.25` and
/// `AvailableBlockRatio = 0.75`, then the target fullness is _0.25 of the
/// normal capacity_ and _0.1875 of the entire block_.
///
/// This implementation implies the bound:
/// - `v ≤ p / k * (s − s')`
/// - or, solving for `p`: `p >= v * k * (s - s')`
///
/// where `p` is the amount of change over `k` blocks.
///
/// Hence:
/// - in a fully congested chain: `p >= v * k * (1 - s')`.
/// - in an empty chain: `p >= v * k * (-s')`.
///
/// For example, when all blocks are full and there are 28800 blocks per day
/// (default in `substrate-node`) and v == 0.00001, s' == 0.1875, we'd have:
///
/// p >= 0.00001 * 28800 * 0.8125
/// p >= 0.234
///
/// Meaning that fees can change by around ~23% per day, given extreme
/// congestion.
///
/// More info can be found at:
/// https://w3f-research.readthedocs.io/en/latest/polkadot/Token%20Economics.html
pub struct TargetedFeeAdjustment<T, S, V, M>(sp_std::marker::PhantomData<(T, S, V, M)>);

/// Something that can convert the current multiplier to the next one.
pub trait MultiplierUpdate: Convert<Multiplier, Multiplier> {
	/// Minimum multiplier
	fn min() -> Multiplier;
	/// Target block saturation level
	fn target() -> Perquintill;
	/// Variability factor
	fn variability() -> Multiplier;
}

impl MultiplierUpdate for () {
	fn min() -> Multiplier {
		Default::default()
	}
	fn target() -> Perquintill {
		Default::default()
	}
	fn variability() -> Multiplier {
		Default::default()
	}
}

impl<T, S, V, M> MultiplierUpdate for TargetedFeeAdjustment<T, S, V, M>
where
	T: frame_system::Config,
	S: Get<Perquintill>,
	V: Get<Multiplier>,
	M: Get<Multiplier>,
{
	fn min() -> Multiplier {
		M::get()
	}
	fn target() -> Perquintill {
		S::get()
	}
	fn variability() -> Multiplier {
		V::get()
	}
}

impl<T, S, V, M> Convert<Multiplier, Multiplier> for TargetedFeeAdjustment<T, S, V, M>
where
	T: frame_system::Config,
	S: Get<Perquintill>,
	V: Get<Multiplier>,
	M: Get<Multiplier>,
{
	fn convert(previous: Multiplier) -> Multiplier {
		// Defensive only. The multiplier in storage should always be at most positive.
		// Nonetheless we recover here in case of errors, because any value below this
		// would be stale and can never change.
		let min_multiplier = M::get();
		let previous = previous.max(min_multiplier);

		let weights = T::BlockWeights::get();
		// the computed ratio is only among the normal class.
		let normal_max_weight = weights
			.get(DispatchClass::Normal)
			.max_total
			.unwrap_or(weights.max_block);
		let current_block_weight = <frame_system::Pallet<T>>::block_weight();
		let normal_block_weight = *current_block_weight.get(DispatchClass::Normal).min(&normal_max_weight);

		let s = S::get();
		let v = V::get();

		let target_weight = (s * normal_max_weight) as u128;
		let block_weight = normal_block_weight as u128;

		// determines if the first_term is positive
		let positive = block_weight >= target_weight;
		let diff_abs = block_weight.max(target_weight) - block_weight.min(target_weight);

		// defensive only, a test case assures that the maximum weight diff can fit in
		// Multiplier without any saturation.
		let diff = Multiplier::saturating_from_rational(diff_abs, normal_max_weight.max(1));
		let diff_squared = diff.saturating_mul(diff);

		let v_squared_2 = v.saturating_mul(v) / Multiplier::saturating_from_integer(2);

		let first_term = v.saturating_mul(diff);
		let second_term = v_squared_2.saturating_mul(diff_squared);

		if positive {
			let excess = first_term.saturating_add(second_term).saturating_mul(previous);
			previous.saturating_add(excess).max(min_multiplier)
		} else {
			// Defensive-only: first_term > second_term. Safe subtraction.
			let negative = first_term.saturating_sub(second_term).saturating_mul(previous);
			previous.saturating_sub(negative).max(min_multiplier)
		}
	}
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub const RESERVE_ID: ReserveIdentifier = ReserveIdentifier::TransactionPayment;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// All non-native currency ids in Acala.
		#[pallet::constant]
		type AllNonNativeCurrencyIds: Get<Vec<CurrencyId>>;

		/// Native currency id, the actual received currency type as fee for
		/// treasury. Should be ACA
		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// Stable currency id, should be AUSD
		#[pallet::constant]
		type StableCurrencyId: Get<CurrencyId>;

		/// The currency type in which fees will be paid.
		type Currency: Currency<Self::AccountId>
			+ NamedReservableCurrency<Self::AccountId, ReserveIdentifier = ReserveIdentifier>
			+ Send
			+ Sync;

		/// Currency to transfer, reserve/unreserve, lock/unlock assets
		type MultiCurrency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Handler for the unbalanced reduction when taking transaction fees.
		/// This is either one or two separate imbalances, the first is the
		/// transaction fee paid, the second is the tip paid, if any.
		type OnTransactionPayment: OnUnbalanced<NegativeImbalanceOf<Self>>;

		/// The fee to be paid for making a transaction; the per-byte portion.
		#[pallet::constant]
		type TransactionByteFee: Get<PalletBalanceOf<Self>>;

		/// Convert a weight value into a deductible fee based on the currency
		/// type.
		type WeightToFee: WeightToFeePolynomial<Balance = PalletBalanceOf<Self>>;

		/// Update the multiplier of the next block, based on the previous
		/// block's weight.
		type FeeMultiplierUpdate: MultiplierUpdate;

		/// DEX to exchange currencies.
		type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

		/// The max slippage allowed when swap fee with DEX
		#[pallet::constant]
		type MaxSlippageSwapWithDEX: Get<Ratio>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::type_value]
	pub fn DefaultFeeMultiplier() -> Multiplier {
		Multiplier::saturating_from_integer(1)
	}

	/// The next fee multiplier.
	///
	/// NextFeeMultiplier: Multiplier
	#[pallet::storage]
	#[pallet::getter(fn next_fee_multiplier)]
	pub type NextFeeMultiplier<T: Config> = StorageValue<_, Multiplier, ValueQuery, DefaultFeeMultiplier>;

	/// The default fee currency for accounts.
	///
	/// DefaultFeeCurrencyId: AccountId => Option<CurrencyId>
	#[pallet::storage]
	#[pallet::getter(fn default_fee_currency_id)]
	pub type DefaultFeeCurrencyId<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, CurrencyId, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		/// `on_initialize` to return the weight used in `on_finalize`.
		fn on_initialize(_: T::BlockNumber) -> Weight {
			<T as Config>::WeightInfo::on_finalize()
		}

		fn on_finalize(_: T::BlockNumber) {
			NextFeeMultiplier::<T>::mutate(|fm| {
				*fm = T::FeeMultiplierUpdate::convert(*fm);
			});
		}

		#[cfg(feature = "std")]
		fn integrity_test() {
			// given weight == u64, we build multipliers from `diff` of two weight values,
			// which can at most be MaximumBlockWeight. Make sure that this can fit in a
			// multiplier without loss.
			use sp_std::convert::TryInto;
			assert!(
				<Multiplier as sp_runtime::traits::Bounded>::max_value()
					>= Multiplier::checked_from_integer(T::BlockWeights::get().max_block.try_into().unwrap()).unwrap(),
			);

			// This is the minimum value of the multiplier. Make sure that if we collapse to
			// this value, we can recover with a reasonable amount of traffic. For this test
			// we assert that if we collapse to minimum, the trend will be positive with a
			// weight value which is 1% more than the target.
			let min_value = T::FeeMultiplierUpdate::min();
			let mut target = T::FeeMultiplierUpdate::target()
				* T::BlockWeights::get().get(DispatchClass::Normal).max_total.expect(
					"Setting `max_total` for `Normal` dispatch class is not compatible with \
					`transaction-payment` module.",
				);

			// add 1 percent;
			let addition = target / 100;
			if addition == 0 {
				// this is most likely because in a test setup we set everything to ().
				return;
			}
			target += addition;

			sp_io::TestExternalities::new_empty().execute_with(|| {
				<frame_system::Pallet<T>>::set_block_consumed_resources(target, 0);
				let next = T::FeeMultiplierUpdate::convert(min_value);
				assert!(
					next > min_value,
					"The minimum bound of the multiplier is too low. When \
					block saturation is more than target by 1% and multiplier is minimal then \
					the multiplier doesn't increase."
				);
			})
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T as Config>::WeightInfo::set_default_fee_token())]
		/// Set default fee token
		pub fn set_default_fee_token(
			origin: OriginFor<T>,
			fee_token: Option<CurrencyId>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			if let Some(currency_id) = fee_token {
				DefaultFeeCurrencyId::<T>::insert(&who, currency_id);
			} else {
				DefaultFeeCurrencyId::<T>::remove(&who);
			}
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T>
where
	PalletBalanceOf<T>: FixedPointOperand,
{
	/// Query the data that we know about the fee of a given `call`.
	///
	/// This module is not and cannot be aware of the internals of a signed
	/// extension, for example a tip. It only interprets the extrinsic as
	/// some encoded value and accounts for its weight and length, the
	/// runtime's extrinsic base weight, and the current fee multiplier.
	///
	/// All dispatchables must be annotated with weight and will have some
	/// fee info. This function always returns.
	pub fn query_info<Extrinsic: GetDispatchInfo>(
		unchecked_extrinsic: Extrinsic,
		len: u32,
	) -> RuntimeDispatchInfo<PalletBalanceOf<T>>
	where
		T: Send + Sync,
		PalletBalanceOf<T>: Send + Sync,
		<T as frame_system::Config>::Call: Dispatchable<Info = DispatchInfo>,
	{
		// NOTE: we can actually make it understand `ChargeTransactionPayment`, but
		// would be some hassle for sure. We have to make it aware of the index of
		// `ChargeTransactionPayment` in `Extra`. Alternatively, we could actually
		// execute the tx's per-dispatch and record the balance of the sender before and
		// after the pipeline.. but this is way too much hassle for a very very little
		// potential gain in the future.
		let dispatch_info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(&unchecked_extrinsic);

		let partial_fee = Self::compute_fee(len, &dispatch_info, 0u32.into());
		let DispatchInfo { weight, class, .. } = dispatch_info;

		RuntimeDispatchInfo {
			weight,
			class,
			partial_fee,
		}
	}

	/// Query the detailed fee of a given `call`.
	pub fn query_fee_details<Extrinsic: GetDispatchInfo>(
		unchecked_extrinsic: Extrinsic,
		len: u32,
	) -> FeeDetails<PalletBalanceOf<T>>
	where
		T::Call: Dispatchable<Info = DispatchInfo>,
	{
		let dispatch_info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(&unchecked_extrinsic);
		Self::compute_fee_details(len, &dispatch_info, 0u32.into())
	}

	/// Compute the fee details for a particular transaction.
	pub fn compute_fee_details(
		len: u32,
		info: &DispatchInfoOf<T::Call>,
		tip: PalletBalanceOf<T>,
	) -> FeeDetails<PalletBalanceOf<T>>
	where
		T::Call: Dispatchable<Info = DispatchInfo>,
	{
		Self::compute_fee_raw(len, info.weight, tip, info.pays_fee, info.class)
	}

	/// Compute the final fee value for a particular transaction.
	///
	/// The final fee is composed of:
	///   - `base_fee`: This is the minimum amount a user pays for a transaction. It is declared as
	///     a base _weight_ in the runtime and converted to a fee using `WeightToFee`.
	///   - `len_fee`: The length fee, the amount paid for the encoded length (in bytes) of the
	///     transaction.
	///   - `weight_fee`: This amount is computed based on the weight of the transaction. Weight
	///     accounts for the execution time of a transaction.
	///   - `targeted_fee_adjustment`: This is a multiplier that can tune the final fee based on the
	///     congestion of the network.
	///   - (Optional) `tip`: If included in the transaction, the tip will be added on top. Only
	///     signed transactions can have a tip.
	///
	/// The base fee and adjusted weight and length fees constitute the
	/// _inclusion fee,_ which is the minimum fee for a transaction to be
	/// included in a block.
	///
	/// ```ignore
	/// inclusion_fee = base_fee + len_fee + [targeted_fee_adjustment * weight_fee];
	/// final_fee = inclusion_fee + tip;
	/// ```
	pub fn compute_fee(
		len: u32,
		info: &DispatchInfoOf<<T as frame_system::Config>::Call>,
		tip: PalletBalanceOf<T>,
	) -> PalletBalanceOf<T>
	where
		<T as frame_system::Config>::Call: Dispatchable<Info = DispatchInfo>,
	{
		Self::compute_fee_details(len, info, tip).final_fee()
	}

	/// Compute the actual post dispatch fee details for a particular
	/// transaction.
	pub fn compute_actual_fee_details(
		len: u32,
		info: &DispatchInfoOf<T::Call>,
		post_info: &PostDispatchInfoOf<T::Call>,
		tip: PalletBalanceOf<T>,
	) -> FeeDetails<PalletBalanceOf<T>>
	where
		T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	{
		Self::compute_fee_raw(
			len,
			post_info.calc_actual_weight(info),
			tip,
			post_info.pays_fee(info),
			info.class,
		)
	}

	/// Compute the actual post dispatch fee for a particular transaction.
	///
	/// Identical to `compute_fee` with the only difference that the post
	/// dispatch corrected weight is used for the weight fee calculation.
	pub fn compute_actual_fee(
		len: u32,
		info: &DispatchInfoOf<<T as frame_system::Config>::Call>,
		post_info: &PostDispatchInfoOf<<T as frame_system::Config>::Call>,
		tip: PalletBalanceOf<T>,
	) -> PalletBalanceOf<T>
	where
		<T as frame_system::Config>::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	{
		Self::compute_actual_fee_details(len, info, post_info, tip).final_fee()
	}

	fn compute_fee_raw(
		len: u32,
		weight: Weight,
		tip: PalletBalanceOf<T>,
		pays_fee: Pays,
		class: DispatchClass,
	) -> FeeDetails<PalletBalanceOf<T>> {
		if pays_fee == Pays::Yes {
			let len = <PalletBalanceOf<T>>::from(len);
			let per_byte = T::TransactionByteFee::get();

			// length fee. this is not adjusted.
			let fixed_len_fee = per_byte.saturating_mul(len);

			// the adjustable part of the fee.
			let unadjusted_weight_fee = Self::weight_to_fee(weight);
			let multiplier = Self::next_fee_multiplier();
			// final adjusted weight fee.
			let adjusted_weight_fee = multiplier.saturating_mul_int(unadjusted_weight_fee);

			let base_fee = Self::weight_to_fee(T::BlockWeights::get().get(class).base_extrinsic);
			FeeDetails {
				inclusion_fee: Some(InclusionFee {
					base_fee,
					len_fee: fixed_len_fee,
					adjusted_weight_fee,
				}),
				tip,
			}
		} else {
			FeeDetails {
				inclusion_fee: None,
				tip,
			}
		}
	}

	fn weight_to_fee(weight: Weight) -> PalletBalanceOf<T> {
		// cap the weight to the maximum defined in runtime, otherwise it will be the
		// `Bounded` maximum of its data type, which is not desired.
		let capped_weight = weight.min(T::BlockWeights::get().max_block);
		T::WeightToFee::calc(&capped_weight)
	}

	pub fn ensure_can_charge_fee(who: &T::AccountId, fee: PalletBalanceOf<T>, reason: WithdrawReasons) {
		let native_currency_id = T::NativeCurrencyId::get();
		let stable_currency_id = T::StableCurrencyId::get();
		let other_currency_ids = T::AllNonNativeCurrencyIds::get();
		let mut charge_fee_order: Vec<CurrencyId> =
			if let Some(default_fee_currency_id) = DefaultFeeCurrencyId::<T>::get(who) {
				vec![vec![default_fee_currency_id, native_currency_id], other_currency_ids].concat()
			} else {
				vec![vec![native_currency_id], other_currency_ids].concat()
			};
		charge_fee_order.dedup();

		let price_impact_limit = Some(T::MaxSlippageSwapWithDEX::get());

		// iterator charge fee order to get enough fee
		for currency_id in charge_fee_order {
			if currency_id == native_currency_id {
				// check native balance if is enough
				let native_is_enough =
					<T as Config>::Currency::free_balance(who)
						.checked_sub(&fee)
						.map_or(false, |new_free_balance| {
							<T as Config>::Currency::ensure_can_withdraw(who, fee, reason, new_free_balance).is_ok()
						});
				if native_is_enough {
					// native balance is enough, break iteration
					break;
				}
			} else {
				// try to use non-native currency to swap native currency by exchange with DEX
				let trading_path = if currency_id == stable_currency_id {
					vec![stable_currency_id, native_currency_id]
				} else {
					vec![currency_id, stable_currency_id, native_currency_id]
				};

				if T::DEX::swap_with_exact_target(
					who,
					&trading_path,
					fee.unique_saturated_into(),
					<T as Config>::MultiCurrency::free_balance(currency_id, who),
					price_impact_limit,
				)
				.is_ok()
				{
					// successfully swap, break iteration
					break;
				}
			}
		}
	}
}

impl<T> Convert<Weight, PalletBalanceOf<T>> for Pallet<T>
where
	T: Config,
	PalletBalanceOf<T>: FixedPointOperand,
{
	/// Compute the fee for the specified weight.
	///
	/// This fee is already adjusted by the per block fee adjustment factor
	/// and is therefore the share that the weight contributes to the
	/// overall fee of a transaction. It is mainly for informational
	/// purposes and not used in the actual fee calculation.
	fn convert(weight: Weight) -> PalletBalanceOf<T> {
		NextFeeMultiplier::<T>::get().saturating_mul_int(Self::weight_to_fee(weight))
	}
}

/// Require the transactor pay for themselves and maybe include a tip to
/// gain additional priority in the queue.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T: Config + Send + Sync>(#[codec(compact)] PalletBalanceOf<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for ChargeTransactionPayment<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "ChargeTransactionPayment<{:?}>", self.0)
	}
	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> ChargeTransactionPayment<T>
where
	<T as frame_system::Config>::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	PalletBalanceOf<T>: Send + Sync + FixedPointOperand,
{
	/// utility constructor. Used only in client/factory code.
	pub fn from(fee: PalletBalanceOf<T>) -> Self {
		Self(fee)
	}

	fn withdraw_fee(
		&self,
		who: &T::AccountId,
		_call: &<T as frame_system::Config>::Call,
		info: &DispatchInfoOf<<T as frame_system::Config>::Call>,
		len: usize,
	) -> Result<(PalletBalanceOf<T>, Option<NegativeImbalanceOf<T>>), TransactionValidityError> {
		let tip = self.0;
		let fee = Pallet::<T>::compute_fee(len as u32, info, tip);

		// Only mess with balances if fee is not zero.
		if fee.is_zero() {
			return Ok((fee, None));
		}

		let reason = if tip.is_zero() {
			WithdrawReasons::TRANSACTION_PAYMENT
		} else {
			WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
		};

		Pallet::<T>::ensure_can_charge_fee(who, fee, reason);

		// withdraw native currency as fee
		match <T as Config>::Currency::withdraw(who, fee, reason, ExistenceRequirement::KeepAlive) {
			Ok(imbalance) => Ok((fee, Some(imbalance))),
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	/// Get an appropriate priority for a transaction with the given length
	/// and info.
	///
	/// This will try and optimise the `fee/weight` `fee/length`, whichever
	/// is consuming more of the maximum corresponding limit.
	///
	/// For example, if a transaction consumed 1/4th of the block length and
	/// half of the weight, its final priority is `fee * min(2, 4) = fee *
	/// 2`. If it consumed `1/4th` of the block length and the entire block
	/// weight `(1/1)`, its priority is `fee * min(1, 4) = fee * 1`. This
	/// means  that the transaction which consumes more resources (either
	/// length or weight) with the same `fee` ends up having lower priority.
	fn get_priority(
		len: usize,
		info: &DispatchInfoOf<<T as frame_system::Config>::Call>,
		final_fee: PalletBalanceOf<T>,
	) -> TransactionPriority {
		let weight_saturation = T::BlockWeights::get().max_block / info.weight.max(1);
		let max_block_length = *T::BlockLength::get().max.get(DispatchClass::Normal);
		let len_saturation = max_block_length as u64 / (len as u64).max(1);
		let coefficient: PalletBalanceOf<T> = weight_saturation
			.min(len_saturation)
			.saturated_into::<PalletBalanceOf<T>>();
		final_fee
			.saturating_mul(coefficient)
			.saturated_into::<TransactionPriority>()
	}
}

impl<T: Config + Send + Sync> SignedExtension for ChargeTransactionPayment<T>
where
	PalletBalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand,
	<T as frame_system::Config>::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
	const IDENTIFIER: &'static str = "ChargeTransactionPayment";
	type AccountId = T::AccountId;
	type Call = <T as frame_system::Config>::Call;
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
		Ok(ValidTransaction {
			priority: Self::get_priority(len, info, fee),
			..Default::default()
		})
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
			let actual_fee = Pallet::<T>::compute_actual_fee(len as u32, info, post_info, tip);
			let refund = fee.saturating_sub(actual_fee);
			let actual_payment = match <T as Config>::Currency::deposit_into_existing(&who, refund) {
				Ok(refund_imbalance) => {
					// The refund cannot be larger than the up front payed max weight.
					// `PostDispatchInfo::calc_unspent` guards against such a case.
					match payed.offset(refund_imbalance) {
						SameOrOther::Same(actual_payment) => actual_payment,
						SameOrOther::None => Default::default(),
						_ => return Err(InvalidTransaction::Payment.into()),
					}
				}
				// We do not recreate the account using the refund. The up front payment
				// is gone in that case.
				Err(_) => payed,
			};
			let (tip, fee) = actual_payment.split(tip);

			// distribute fee
			<T as Config>::OnTransactionPayment::on_unbalanceds(Some(fee).into_iter().chain(Some(tip)));
		}
		Ok(())
	}
}

impl<T: Config + Send + Sync> TransactionPayment<T::AccountId, PalletBalanceOf<T>, NegativeImbalanceOf<T>>
	for ChargeTransactionPayment<T>
where
	PalletBalanceOf<T>: Send + Sync + FixedPointOperand,
{
	fn reserve_fee(who: &T::AccountId, weight: Weight) -> Result<PalletBalanceOf<T>, DispatchError> {
		let fee = Pallet::<T>::weight_to_fee(weight);
		Pallet::<T>::ensure_can_charge_fee(who, fee, WithdrawReasons::TRANSACTION_PAYMENT);
		<T as Config>::Currency::reserve_named(&RESERVE_ID, &who, fee)?;
		Ok(fee)
	}

	fn unreserve_fee(who: &T::AccountId, fee: PalletBalanceOf<T>) {
		<T as Config>::Currency::unreserve_named(&RESERVE_ID, &who, fee);
	}

	fn unreserve_and_charge_fee(
		who: &T::AccountId,
		weight: Weight,
	) -> Result<(PalletBalanceOf<T>, NegativeImbalanceOf<T>), TransactionValidityError> {
		let fee = Pallet::<T>::weight_to_fee(weight);
		<T as Config>::Currency::unreserve_named(&RESERVE_ID, &who, fee);

		match <T as Config>::Currency::withdraw(
			who,
			fee,
			WithdrawReasons::TRANSACTION_PAYMENT,
			ExistenceRequirement::KeepAlive,
		) {
			Ok(imbalance) => Ok((fee, imbalance)),
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	fn refund_fee(
		who: &T::AccountId,
		refund_weight: Weight,
		payed: NegativeImbalanceOf<T>,
	) -> Result<(), TransactionValidityError> {
		let refund = Pallet::<T>::weight_to_fee(refund_weight);
		let actual_payment = match <T as Config>::Currency::deposit_into_existing(&who, refund) {
			Ok(refund_imbalance) => {
				// The refund cannot be larger than the up front payed max weight.
				match payed.offset(refund_imbalance) {
					SameOrOther::Same(actual_payment) => actual_payment,
					SameOrOther::None => Default::default(),
					_ => return Err(InvalidTransaction::Payment.into()),
				}
			}
			// We do not recreate the account using the refund. The up front payment
			// is gone in that case.
			Err(_) => payed,
		};

		// distribute fee
		<T as Config>::OnTransactionPayment::on_unbalanced(actual_payment);

		Ok(())
	}

	fn charge_fee(
		who: &T::AccountId,
		len: u32,
		weight: Weight,
		tip: PalletBalanceOf<T>,
		pays_fee: Pays,
		class: DispatchClass,
	) -> Result<(), TransactionValidityError> {
		let fee = Pallet::<T>::compute_fee_raw(len, weight, tip, pays_fee, class).final_fee();

		Pallet::<T>::ensure_can_charge_fee(who, fee, WithdrawReasons::TRANSACTION_PAYMENT);

		// withdraw native currency as fee
		let actual_payment = <T as Config>::Currency::withdraw(
			who,
			fee,
			WithdrawReasons::TRANSACTION_PAYMENT,
			ExistenceRequirement::KeepAlive,
		)
		.map_err(|_| InvalidTransaction::Payment)?;

		// distribute fee
		<T as Config>::OnTransactionPayment::on_unbalanced(actual_payment);
		Ok(())
	}
}
