// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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
	transactional,
	weights::{
		constants::WEIGHT_PER_SECOND, DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo, WeightToFeeCoefficient,
		WeightToFeePolynomial,
	},
	BoundedVec, PalletId,
};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, InclusionFee};
use primitives::{Balance, CurrencyId, ReserveIdentifier};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		AccountIdConversion, Convert, DispatchInfoOf, One, PostDispatchInfoOf, SaturatedConversion, Saturating,
		SignedExtension, Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	FixedPointNumber, FixedPointOperand, FixedU128, Perquintill,
};
use sp_std::prelude::*;
use support::{DEXManager, PriceProvider, Ratio, SwapLimit, TransactionPayment};
use xcm::opaque::latest::{prelude::XcmError, AssetId, Fungibility::Fungible, MultiAsset, MultiLocation};
use xcm_builder::TakeRevenue;
use xcm_executor::{traits::WeightTrader, Assets};

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
	pub const DEPOSIT_ID: ReserveIdentifier = ReserveIdentifier::TransactionPaymentDeposit;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Native currency id, the actual received currency type as fee for
		/// treasury. Should be ACA
		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// Default fee swap path list
		#[pallet::constant]
		type DefaultFeeSwapPathList: Get<Vec<Vec<CurrencyId>>>;

		/// The currency type in which fees will be paid.
		type Currency: Currency<Self::AccountId, Balance = Balance>
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

		/// A fee mulitplier for `Operational` extrinsics to compute "virtual tip" to boost their
		/// `priority`
		///
		/// This value is multipled by the `final_fee` to obtain a "virtual tip" that is later
		/// added to a tip component in regular `priority` calculations.
		/// It means that a `Normal` transaction can front-run a similarly-sized `Operational`
		/// extrinsic (with no tip), by including a tip value greater than the virtual tip.
		///
		/// ```rust,ignore
		/// // For `Normal`
		/// let priority = priority_calc(tip);
		///
		/// // For `Operational`
		/// let virtual_tip = (inclusion_fee + tip) * OperationalFeeMultiplier;
		/// let priority = priority_calc(tip + virtual_tip);
		/// ```
		///
		/// Note that since we use `final_fee` the multiplier applies also to the regular `tip`
		/// sent with the transaction. So, not only does the transaction get a priority bump based
		/// on the `inclusion_fee`, but we also amplify the impact of tips applied to `Operational`
		/// transactions.
		#[pallet::constant]
		type OperationalFeeMultiplier: Get<u64>;

		/// The step amount of tips required to effect transaction priority.
		#[pallet::constant]
		type TipPerWeightStep: Get<PalletBalanceOf<Self>>;

		/// The maximum value of tips that affect the priority.
		/// Set the maximum value of tips to prevent affecting the unsigned extrinsic.
		#[pallet::constant]
		type MaxTipsOfPriority: Get<PalletBalanceOf<Self>>;

		/// Deposit for setting an Alternative fee swap
		#[pallet::constant]
		type AlternativeFeeSwapDeposit: Get<PalletBalanceOf<Self>>;

		/// Convert a weight value into a deductible fee based on the currency
		/// type.
		type WeightToFee: WeightToFeePolynomial<Balance = PalletBalanceOf<Self>>;

		/// Update the multiplier of the next block, based on the previous
		/// block's weight.
		type FeeMultiplierUpdate: MultiplierUpdate;

		/// DEX to exchange currencies.
		type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

		/// When swap with DEX, the acceptable max slippage for the price from oracle.
		#[pallet::constant]
		type MaxSwapSlippageCompareToOracle: Get<Ratio>;

		/// The limit for length of trading path
		#[pallet::constant]
		type TradingPathLimit: Get<u32>;

		/// The price source to provider external market price.
		type PriceSource: PriceProvider<CurrencyId>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// PalletId used to derivate sub account.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Treasury account used to transfer balance to sub account of `PalletId`.
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		/// The origin which change swap balance threshold or enable charge fee pool.
		type UpdateOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
	}

	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		//TODO: rename to snake case after https://github.com/paritytech/substrate/issues/8826 fixed.
		#[allow(non_snake_case)]
		/// The polynomial that is applied in order to derive fee from weight.
		fn WeightToFee() -> Vec<WeightToFeeCoefficient<PalletBalanceOf<T>>> {
			T::WeightToFee::polynomial().to_vec()
		}
	}

	#[pallet::type_value]
	pub fn DefaultFeeMultiplier() -> Multiplier {
		Multiplier::saturating_from_integer(1)
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The swap path is invalid
		InvalidSwapPath,
		/// The balance is invalid
		InvalidBalance,
		/// Can't find rate by the supply token
		InvalidRate,
		/// Dex swap pool is not available now
		DexNotAvailable,
		/// Charge fee pool is already exist
		ChargeFeePoolAlreadyExisted,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The threshold balance that trigger swap from dex was updated.
		SwapBalanceThresholdUpdated {
			currency_id: CurrencyId,
			swap_threshold: Balance,
		},
		/// The charge fee pool is enabled
		ChargeFeePoolEnabled {
			sub_account: T::AccountId,
			currency_id: CurrencyId,
			exchange_rate: Ratio,
			pool_size: Balance,
			swap_threshold: Balance,
		},
	}

	/// The next fee multiplier.
	///
	/// NextFeeMultiplier: Multiplier
	#[pallet::storage]
	#[pallet::getter(fn next_fee_multiplier)]
	pub type NextFeeMultiplier<T: Config> = StorageValue<_, Multiplier, ValueQuery, DefaultFeeMultiplier>;

	/// The alternative fee swap path of accounts.
	#[pallet::storage]
	#[pallet::getter(fn alternative_fee_swap_path)]
	pub type AlternativeFeeSwapPath<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<CurrencyId, T::TradingPathLimit>, OptionQuery>;

	/// The size of fee pool in native token. During `initialize_pool` this amount of native token
	/// will be transferred from `TreasuryAccount` to sub account of `PalletId`.
	#[pallet::storage]
	#[pallet::getter(fn pool_size)]
	pub type PoolSize<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Balance, ValueQuery>;

	/// The exchange rate between the given currency and native token.
	/// This value is updated when upon swap from dex.
	#[pallet::storage]
	#[pallet::getter(fn token_exchange_rate)]
	pub type TokenExchangeRate<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Ratio, OptionQuery>;

	/// The balance threshold to trigger swap from dex, normally the value is gt ED of native asset.
	#[pallet::storage]
	#[pallet::getter(fn swap_balance_threshold)]
	pub type SwapBalanceThreshold<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Balance, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
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
		/// Set fee swap path
		#[pallet::weight(<T as Config>::WeightInfo::set_alternative_fee_swap_path())]
		pub fn set_alternative_fee_swap_path(
			origin: OriginFor<T>,
			fee_swap_path: Option<Vec<CurrencyId>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if let Some(path) = fee_swap_path {
				let path: BoundedVec<CurrencyId, T::TradingPathLimit> =
					path.try_into().map_err(|_| Error::<T>::InvalidSwapPath)?;
				ensure!(
					path.len() > 1
						&& path[0] != T::NativeCurrencyId::get()
						&& path[path.len() - 1] == T::NativeCurrencyId::get(),
					Error::<T>::InvalidSwapPath
				);
				AlternativeFeeSwapPath::<T>::insert(&who, &path);
				T::Currency::ensure_reserved_named(&DEPOSIT_ID, &who, T::AlternativeFeeSwapDeposit::get())?;
			} else {
				AlternativeFeeSwapPath::<T>::remove(&who);
				T::Currency::unreserve_all_named(&DEPOSIT_ID, &who);
			}
			Ok(())
		}

		/// Set swap balance threshold of native asset
		#[pallet::weight(<T as Config>::WeightInfo::set_swap_balance_threshold())]
		pub fn set_swap_balance_threshold(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			swap_threshold: Balance,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			ensure!(
				swap_threshold < PoolSize::<T>::get(currency_id),
				Error::<T>::InvalidBalance
			);
			SwapBalanceThreshold::<T>::insert(currency_id, swap_threshold);
			Self::deposit_event(Event::SwapBalanceThresholdUpdated {
				currency_id,
				swap_threshold,
			});
			Ok(())
		}

		/// Enable and initialize charge fee pool.
		#[pallet::weight(<T as Config>::WeightInfo::enable_charge_fee_pool())]
		pub fn enable_charge_fee_pool(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			pool_size: Balance,
			swap_threshold: Balance,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::initialize_pool(currency_id, pool_size, swap_threshold)
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
		let native_existential_deposit = <T as Config>::Currency::minimum_balance();
		let total_native = <T as Config>::Currency::total_balance(who);

		// check native balance if is enough
		let native_is_enough = fee.saturating_add(native_existential_deposit) <= total_native
			&& <T as Config>::Currency::free_balance(who)
				.checked_sub(fee)
				.map_or(false, |new_free_balance| {
					<T as Config>::Currency::ensure_can_withdraw(who, fee, reason, new_free_balance).is_ok()
				});
		if native_is_enough {
			return;
		}

		// make sure add extra gap to keep alive after swap.
		let amount = fee.saturating_add(native_existential_deposit.saturating_sub(total_native));
		// native is not enough, try swap native from fee pool to pay fee and gap.
		Self::swap_native_asset(who, amount);
	}

	/// Iterate order list, break if can swap out enough native asset amount with user's foreign
	/// asset. make sure trading path is exist in dex, if the trading pair is not exist in dex, even
	/// though we have setup it in charge fee pool, we can't charge fee with this foreign asset.
	fn swap_native_asset(who: &T::AccountId, amount: Balance) {
		let native_currency_id = T::NativeCurrencyId::get();
		for trading_path in Self::get_trading_path(who) {
			if let Some(target_currency_id) = trading_path.last() {
				if *target_currency_id == native_currency_id {
					let supply_currency_id = *trading_path.first().expect("should match a non native asset");
					if Self::swap_from_pool_or_dex(who, amount, supply_currency_id).is_ok() {
						break;
					}
				}
			}
		}
	}

	/// swap user's given asset with native asset. prior exchange from charge fee pool, if native
	/// asset balance of charge fee pool is not enough, swap from dex.
	#[transactional]
	fn swap_from_pool_or_dex(who: &T::AccountId, amount: Balance, supply_currency_id: CurrencyId) -> DispatchResult {
		let rate = TokenExchangeRate::<T>::get(supply_currency_id).ok_or(Error::<T>::InvalidRate)?;
		let sub_account = Self::sub_account_id(supply_currency_id);

		// if sub account has not enough native asset, trigger swap from dex. if `native_balance`
		// is lt ED, it become 0 because we don't add sub account to whitelist on purpose,
		// this means the charge fee pool is exhausted for this given token pair.
		// we normally set the `SwapBalanceThreshold` gt ED to prevent this case.
		let native_balance = T::Currency::free_balance(&sub_account);
		if native_balance < SwapBalanceThreshold::<T>::get(supply_currency_id) {
			let trading_path = Self::get_trading_path_by_currency(&sub_account, supply_currency_id);
			if let Some(trading_path) = trading_path {
				let supply_balance = T::MultiCurrency::free_balance(supply_currency_id, &sub_account);
				let supply_amount =
					supply_balance.saturating_sub(T::MultiCurrency::minimum_balance(supply_currency_id));
				if let Ok((_, swap_native_balance)) = T::DEX::swap_with_specific_path(
					&sub_account,
					&trading_path,
					SwapLimit::ExactSupply(supply_amount, 0),
				) {
					// calculate and update new rate, also update the pool size
					let new_pool_size = swap_native_balance.saturating_add(native_balance);
					let new_native_balance = rate.saturating_mul_int(new_pool_size);
					let next_updated_rate =
						Ratio::saturating_from_rational(new_native_balance, PoolSize::<T>::get(supply_currency_id));
					TokenExchangeRate::<T>::insert(supply_currency_id, next_updated_rate);
					PoolSize::<T>::insert(supply_currency_id, new_pool_size);
				} else {
					debug_assert!(false, "Swap tx fee pool should not fail!");
				}
			}
		}

		// use fix rate to calculate the amount of supply asset that equal to native asset.
		let supply_account = rate.saturating_mul_int(amount);
		T::MultiCurrency::transfer(supply_currency_id, who, &sub_account, supply_account)?;
		T::Currency::transfer(&sub_account, who, amount, ExistenceRequirement::KeepAlive)?;
		Ok(())
	}

	/// Get trading path by user.
	fn get_trading_path(who: &T::AccountId) -> Vec<Vec<CurrencyId>> {
		let mut default_fee_swap_path_list = T::DefaultFeeSwapPathList::get();
		if let Some(trading_path) = AlternativeFeeSwapPath::<T>::get(who) {
			default_fee_swap_path_list.insert(0, trading_path.into_inner())
		}
		default_fee_swap_path_list
	}

	/// Get trading path by user and supply asset.
	pub fn get_trading_path_by_currency(who: &T::AccountId, supply_currency_id: CurrencyId) -> Option<Vec<CurrencyId>> {
		let fee_swap_path_list: Vec<Vec<CurrencyId>> = Self::get_trading_path(who);
		for trading_path in fee_swap_path_list {
			if let Some(currency) = trading_path.first() {
				if *currency == supply_currency_id {
					return Some(trading_path);
				}
			}
		}
		None
	}

	/// The sub account derivated by `PalletId`.
	fn sub_account_id(id: CurrencyId) -> T::AccountId {
		T::PalletId::get().into_sub_account(id)
	}

	/// Initiate a charge fee swap pool. Usually used in `on_runtime_upgrade` or manual
	/// `enable_charge_fee_pool` dispatch call.
	#[transactional]
	pub fn initialize_pool(currency_id: CurrencyId, pool_size: Balance, swap_threshold: Balance) -> DispatchResult {
		let treasury_account = T::TreasuryAccount::get();
		let sub_account = Self::sub_account_id(currency_id);
		let native_existential_deposit = <T as Config>::Currency::minimum_balance();
		ensure!(
			pool_size > native_existential_deposit && pool_size > swap_threshold,
			Error::<T>::InvalidBalance
		);
		ensure!(
			PoolSize::<T>::get(currency_id).is_zero(),
			Error::<T>::ChargeFeePoolAlreadyExisted
		);

		let trading_path = Self::get_trading_path_by_currency(&sub_account, currency_id);
		if let Some(trading_path) = trading_path {
			let (supply_amount, _) = T::DEX::get_swap_amount(
				&trading_path,
				SwapLimit::ExactTarget(Balance::MAX, native_existential_deposit),
			)
			.ok_or(Error::<T>::DexNotAvailable)?;
			let exchange_rate = Ratio::saturating_from_rational(supply_amount, native_existential_deposit);

			T::MultiCurrency::transfer(
				currency_id,
				&treasury_account,
				&sub_account,
				T::MultiCurrency::minimum_balance(currency_id),
			)?;
			T::Currency::transfer(
				&treasury_account,
				&sub_account,
				pool_size,
				ExistenceRequirement::KeepAlive,
			)?;

			SwapBalanceThreshold::<T>::insert(currency_id, swap_threshold);
			TokenExchangeRate::<T>::insert(currency_id, exchange_rate);
			PoolSize::<T>::insert(currency_id, pool_size);
			Self::deposit_event(Event::ChargeFeePoolEnabled {
				sub_account,
				currency_id,
				exchange_rate,
				pool_size,
				swap_threshold,
			});
		}
		Ok(())
	}
}

/// `WeightTrader` implementation used for `Trader`, the `rate` is read from storage,
/// and `token_per_second` is calculated by `rate` * `native_asset_per_second`.
pub struct TransactionFeePoolTrader<T, C, K: Get<u128>, R: TakeRevenue> {
	weight: Weight,
	amount: u128,
	asset_location: Option<MultiLocation>,
	asset_per_second: u128,
	_marker: PhantomData<(T, C, K, R)>,
}

impl<T: Config, C, K: Get<u128>, R: TakeRevenue> WeightTrader for TransactionFeePoolTrader<T, C, K, R>
where
	C: Convert<MultiLocation, Option<CurrencyId>>,
{
	fn new() -> Self {
		Self {
			weight: 0,
			amount: 0,
			asset_location: None,
			asset_per_second: 0,
			_marker: Default::default(),
		}
	}

	fn buy_weight(&mut self, weight: Weight, payment: Assets) -> Result<Assets, XcmError> {
		// only support first fungible assets now.
		let asset_id = payment
			.fungible
			.iter()
			.next()
			.map_or(Err(XcmError::TooExpensive), |v| Ok(v.0))?;

		if let AssetId::Concrete(ref multi_location) = asset_id.clone() {
			if let Some(token_id) = C::convert(multi_location.clone()) {
				if let Some(rate) = TokenExchangeRate::<T>::get(token_id) {
					// calculate the amount of fungible asset.
					let weight_ratio = Ratio::saturating_from_rational(weight as u128, WEIGHT_PER_SECOND as u128);
					let asset_per_second = rate.saturating_mul_int(K::get());
					let amount = weight_ratio.saturating_mul_int(asset_per_second);
					let required = MultiAsset {
						id: asset_id.clone(),
						fun: Fungible(amount),
					};
					let unused = payment.checked_sub(required).map_err(|_| XcmError::TooExpensive)?;
					self.weight = self.weight.saturating_add(weight);
					self.amount = self.amount.saturating_add(amount);
					self.asset_location = Some(multi_location.clone());
					self.asset_per_second = asset_per_second;
					return Ok(unused);
				}
			}
		}
		Err(XcmError::TooExpensive)
	}

	fn refund_weight(&mut self, weight: Weight) -> Option<MultiAsset> {
		let weight = weight.min(self.weight);
		let weight_ratio = Ratio::saturating_from_rational(weight as u128, WEIGHT_PER_SECOND as u128);
		let amount = weight_ratio.saturating_mul_int(self.asset_per_second);
		self.weight = self.weight.saturating_sub(weight);
		self.amount = self.amount.saturating_sub(amount);
		if amount > 0 && self.asset_location.is_some() {
			Some(
				(
					self.asset_location.as_ref().expect("checked is non-empty; qed").clone(),
					amount,
				)
					.into(),
			)
		} else {
			None
		}
	}
}

impl<T, C, K: Get<u128>, R: TakeRevenue> Drop for TransactionFeePoolTrader<T, C, K, R> {
	fn drop(&mut self) {
		if self.amount > 0 && self.asset_location.is_some() {
			R::take_revenue(
				(
					self.asset_location.as_ref().expect("checked is non-empty; qed").clone(),
					self.amount,
				)
					.into(),
			);
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
///
/// # Transaction Validity
///
/// This extension sets the `priority` field of `TransactionValidity` depending on the amount
/// of tip being paid per weight unit.
///
/// Operational transactions will receive an additional priority bump, so that they are normally
/// considered before regular transactions.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ChargeTransactionPayment<T: Config + Send + Sync>(#[codec(compact)] pub PalletBalanceOf<T>);

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

	/// Get an appropriate priority for a transaction with the given `DispatchInfo`, encoded length
	/// and user-included tip.
	///
	/// The priority is based on the amount of `tip` the user is willing to pay per unit of either
	/// `weight` or `length`, depending which one is more limitting. For `Operational` extrinsics
	/// we add a "virtual tip" to the calculations.
	///
	/// The formula should simply be `tip / bounded_{weight|length}`, but since we are using
	/// integer division, we have no guarantees it's going to give results in any reasonable
	/// range (might simply end up being zero). Hence we use a scaling factor:
	/// `tip * (max_block_{weight|length} / bounded_{weight|length})`, since given current
	/// state of-the-art blockchains, number of per-block transactions is expected to be in a
	/// range reasonable enough to not saturate the `Balance` type while multiplying by the tip.
	fn get_priority(
		info: &DispatchInfoOf<<T as frame_system::Config>::Call>,
		len: usize,
		tip: PalletBalanceOf<T>,
		final_fee: PalletBalanceOf<T>,
	) -> TransactionPriority {
		// Calculate how many such extrinsics we could fit into an empty block and take
		// the limitting factor.
		let max_block_weight = T::BlockWeights::get().max_block;
		let max_block_length = *T::BlockLength::get().max.get(info.class) as u64;

		let bounded_weight = info.weight.max(1).min(max_block_weight);
		let bounded_length = (len as u64).max(1).min(max_block_length);

		let max_tx_per_block_weight = max_block_weight / bounded_weight;
		let max_tx_per_block_length = max_block_length / bounded_length;
		// Given our current knowledge this value is going to be in a reasonable range - i.e.
		// less than 10^9 (2^30), so multiplying by the `tip` value is unlikely to overflow the
		// balance type. We still use saturating ops obviously, but the point is to end up with some
		// `priority` distribution instead of having all transactions saturate the priority.
		let max_tx_per_block = max_tx_per_block_length
			.min(max_tx_per_block_weight)
			.saturated_into::<PalletBalanceOf<T>>();
		// tipPerWeight = tipPerWight / TipPerWeightStep * TipPerWeightStep
		//              = tip / bounded_{weight|length} / TipPerWeightStep * TipPerWeightStep
		// priority = tipPerWeight * max_block_{weight|length}
		// MaxTipsOfPriority = 10_000 KAR/ACA = 10^16.
		// `MaxTipsOfPriority * max_block_{weight|length}` will overflow, so div `TipPerWeightStep` here.
		let max_reward = |val: PalletBalanceOf<T>| {
			val.checked_div(T::TipPerWeightStep::get())
				.expect("TipPerWeightStep is non-zero; qed")
				.saturating_mul(max_tx_per_block)
		};

		// To distribute no-tip transactions a little bit, we increase the tip value by one.
		// This means that given two transactions without a tip, smaller one will be preferred.
		// Set the maximum value of tips to prevent affecting the unsigned extrinsic.
		let tip = tip.saturating_add(One::one()).min(T::MaxTipsOfPriority::get());
		let scaled_tip = max_reward(tip);

		match info.class {
			DispatchClass::Normal => {
				// For normal class we simply take the `tip_per_weight`.
				scaled_tip
			}
			DispatchClass::Mandatory => {
				// Mandatory extrinsics should be prohibited (e.g. by the [`CheckWeight`]
				// extensions), but just to be safe let's return the same priority as `Normal` here.
				scaled_tip
			}
			DispatchClass::Operational => {
				// A "virtual tip" value added to an `Operational` extrinsic.
				// This value should be kept high enough to allow `Operational` extrinsics
				// to get in even during congestion period, but at the same time low
				// enough to prevent a possible spam attack by sending invalid operational
				// extrinsics which push away regular transactions from the pool.
				let fee_multiplier = T::OperationalFeeMultiplier::get().saturated_into();
				let virtual_tip = final_fee.saturating_mul(fee_multiplier);
				let scaled_virtual_tip = max_reward(virtual_tip);

				scaled_tip.saturating_add(scaled_virtual_tip)
			}
		}
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
		let (final_fee, _) = self.withdraw_fee(who, call, info, len)?;
		let tip = self.0;
		Ok(ValidTransaction {
			priority: Self::get_priority(info, len, tip, final_fee),
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
		pre: Option<Self::Pre>,
		info: &DispatchInfoOf<Self::Call>,
		post_info: &PostDispatchInfoOf<Self::Call>,
		len: usize,
		_result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		if let Some((tip, who, Some(payed), fee)) = pre {
			let actual_fee = Pallet::<T>::compute_actual_fee(len as u32, info, post_info, tip);
			let refund_fee = fee.saturating_sub(actual_fee);
			let mut refund = refund_fee;
			let mut actual_tip = tip;

			if !tip.is_zero() && !info.weight.is_zero() {
				// tip_pre_weight * unspent_weight
				let refund_tip = tip
					.checked_div(info.weight.saturated_into::<PalletBalanceOf<T>>())
					.expect("checked is non-zero; qed")
					.saturating_mul(post_info.calc_unspent(info).saturated_into::<PalletBalanceOf<T>>());
				refund = refund_fee.saturating_add(refund_tip);
				actual_tip = tip.saturating_sub(refund_tip);
			}
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
			let (tip, fee) = actual_payment.split(actual_tip);

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
		<T as Config>::Currency::reserve_named(&RESERVE_ID, who, fee)?;
		Ok(fee)
	}

	fn unreserve_fee(who: &T::AccountId, fee: PalletBalanceOf<T>) {
		<T as Config>::Currency::unreserve_named(&RESERVE_ID, who, fee);
	}

	fn unreserve_and_charge_fee(
		who: &T::AccountId,
		weight: Weight,
	) -> Result<(PalletBalanceOf<T>, NegativeImbalanceOf<T>), TransactionValidityError> {
		let fee = Pallet::<T>::weight_to_fee(weight);
		<T as Config>::Currency::unreserve_named(&RESERVE_ID, who, fee);

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
		let actual_payment = match <T as Config>::Currency::deposit_into_existing(who, refund) {
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
