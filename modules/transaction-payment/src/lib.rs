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
#![allow(clippy::boxed_local)]
#![allow(clippy::type_complexity)]

use frame_support::{
	dispatch::{DispatchResult, Dispatchable},
	pallet_prelude::*,
	traits::{
		Currency, ExistenceRequirement, Imbalance, IsSubType, NamedReservableCurrency, OnUnbalanced, SameOrOther,
		WithdrawReasons,
	},
	transactional,
	weights::{DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo, WeightToFee},
	BoundedVec, PalletId,
};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, InclusionFee};
use primitives::{Balance, CurrencyId, Multiplier, ReserveIdentifier};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		AccountIdConversion, Convert, DispatchInfoOf, One, PostDispatchInfoOf, SaturatedConversion, Saturating,
		SignedExtension, Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	FixedPointNumber, FixedPointOperand, MultiSignature, Percent, Perquintill,
};
use sp_std::prelude::*;
use support::{BuyWeightRate, DEXManager, PriceProvider, Ratio, SwapLimit, TransactionPayment};
use xcm::opaque::latest::MultiLocation;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

type PalletBalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;
type CallOf<T> = <T as Config>::Call;

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

		/// The aggregated call type.
		type Call: Parameter
			+ Dispatchable<Origin = Self::Origin, PostInfo = PostDispatchInfo, Info = DispatchInfo>
			+ GetDispatchInfo
			+ IsSubType<Call<Self>>
			+ IsType<<Self as frame_system::Config>::Call>;

		/// Native currency id, the actual received currency type as fee for
		/// treasury. Should be ACA
		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// The currency type in which fees will be paid.
		type Currency: NamedReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = ReserveIdentifier,
			Balance = Balance,
		>;

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
		type WeightToFee: WeightToFee<Balance = PalletBalanceOf<Self>>;

		/// Update the multiplier of the next block, based on the previous
		/// block's weight.
		type FeeMultiplierUpdate: MultiplierUpdate;

		/// DEX to exchange currencies.
		type DEX: DEXManager<Self::AccountId, Balance, CurrencyId>;

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

		/// Custom fee surplus if not payed with native asset.
		#[pallet::constant]
		type CustomFeeSurplus: Get<Percent>;

		/// Alternative fee surplus if not payed with native asset.
		#[pallet::constant]
		type AlternativeFeeSurplus: Get<Percent>;

		/// Default fee tokens used in tx fee pool.
		#[pallet::constant]
		type DefaultFeeTokens: Get<Vec<CurrencyId>>;

		/// The origin which change swap balance threshold or enable charge fee pool.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;
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
		/// Can't find the token info in the charge fee pool
		InvalidToken,
		/// Dex swap pool is not available now
		DexNotAvailable,
		/// Charge fee pool is already exist
		ChargeFeePoolAlreadyExisted,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// The charge fee pool is enabled
		ChargeFeePoolEnabled {
			sub_account: T::AccountId,
			currency_id: CurrencyId,
			fee_swap_path: Vec<CurrencyId>,
			exchange_rate: Ratio,
			pool_size: Balance,
			swap_threshold: Balance,
		},
		/// The charge fee pool is swapped
		ChargeFeePoolSwapped {
			sub_account: T::AccountId,
			supply_currency_id: CurrencyId,
			old_exchange_rate: Ratio,
			swap_exchange_rate: Ratio,
			new_exchange_rate: Ratio,
			new_pool_size: Balance,
		},
		/// The charge fee pool is disabled
		ChargeFeePoolDisabled {
			currency_id: CurrencyId,
			foreign_amount: Balance,
			native_amount: Balance,
		},
	}

	/// The next fee multiplier.
	///
	/// NextFeeMultiplier: Multiplier
	#[pallet::storage]
	#[pallet::getter(fn next_fee_multiplier)]
	pub type NextFeeMultiplier<T: Config> = StorageValue<_, Multiplier, ValueQuery, DefaultFeeMultiplier>;

	/// The alternative fee swap path of accounts.
	///
	/// AlternativeFeeSwapPath: map AccountId => Option<Vec<CurrencyId>>
	#[pallet::storage]
	#[pallet::getter(fn alternative_fee_swap_path)]
	pub type AlternativeFeeSwapPath<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<CurrencyId, T::TradingPathLimit>, OptionQuery>;

	/// The global fee swap path.
	/// The path includes `DefaultFeeTokens` trading path, and foreign asset trading path.
	///
	/// GlobalFeeSwapPath: map CurrencyId => Option<Vec<CurrencyId>>
	#[pallet::storage]
	#[pallet::getter(fn global_fee_swap_path)]
	pub type GlobalFeeSwapPath<T: Config> =
		StorageMap<_, Twox64Concat, CurrencyId, BoundedVec<CurrencyId, T::TradingPathLimit>, OptionQuery>;

	/// The size of fee pool in native token. During `initialize_pool` this amount of native token
	/// will be transferred from `TreasuryAccount` to sub account of `PalletId`.
	///
	/// PoolSize: map CurrencyId => Balance
	#[pallet::storage]
	#[pallet::getter(fn pool_size)]
	pub type PoolSize<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Balance, ValueQuery>;

	/// The exchange rate between the given currency and native token.
	/// This value is updated when upon swap from dex.
	///
	/// TokenExchangeRate: map CurrencyId => Option<Ratio>
	#[pallet::storage]
	#[pallet::getter(fn token_exchange_rate)]
	pub type TokenExchangeRate<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Ratio, OptionQuery>;

	/// The balance threshold to trigger swap from dex, normally the value is gt ED of native asset.
	///
	/// SwapBalanceThreshold: map CurrencyId => Balance
	#[pallet::storage]
	#[pallet::getter(fn swap_balance_threshold)]
	pub type SwapBalanceThreshold<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Balance, ValueQuery>;

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
			assert!(
				<Multiplier as sp_runtime::traits::Bounded>::max_value()
					>= Multiplier::checked_from_integer::<u128>(T::BlockWeights::get().max_block.try_into().unwrap())
						.unwrap(),
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
		#[transactional]
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
						&& path.first() != Some(&T::NativeCurrencyId::get())
						&& path.last() == Some(&T::NativeCurrencyId::get()),
					Error::<T>::InvalidSwapPath
				);
				T::Currency::ensure_reserved_named(&DEPOSIT_ID, &who, T::AlternativeFeeSwapDeposit::get())?;
				AlternativeFeeSwapPath::<T>::insert(&who, &path);
			} else {
				AlternativeFeeSwapPath::<T>::remove(&who);
				T::Currency::unreserve_all_named(&DEPOSIT_ID, &who);
			}
			Ok(())
		}

		/// Enable and initialize charge fee pool.
		#[pallet::weight(<T as Config>::WeightInfo::enable_charge_fee_pool())]
		#[transactional]
		pub fn enable_charge_fee_pool(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			swap_path: Vec<CurrencyId>,
			pool_size: Balance,
			swap_threshold: Balance,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::initialize_pool(currency_id, swap_path, pool_size, swap_threshold)
		}

		/// Disable charge fee pool.
		#[pallet::weight(<T as Config>::WeightInfo::disable_charge_fee_pool())]
		#[transactional]
		pub fn disable_charge_fee_pool(origin: OriginFor<T>, currency_id: CurrencyId) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::disable_pool(currency_id)
		}

		/// Dapp wrap call, and user pay tx fee as provided trading path. this dispatch call should
		/// make sure the trading path is valid.
		#[pallet::weight({
			let dispatch_info = call.get_dispatch_info();
			(T::WeightInfo::with_fee_path().saturating_add(dispatch_info.weight), dispatch_info.class,)
		})]
		pub fn with_fee_path(
			origin: OriginFor<T>,
			_fee_swap_path: Vec<CurrencyId>,
			call: Box<CallOf<T>>,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin.clone())?;
			call.dispatch(origin)
		}

		/// Dapp wrap call, and user pay tx fee as provided currency, this dispatch call should make
		/// sure the currency is exist in tx fee pool.
		#[pallet::weight({
			let dispatch_info = call.get_dispatch_info();
			(T::WeightInfo::with_fee_currency().saturating_add(dispatch_info.weight), dispatch_info.class,)
		})]
		pub fn with_fee_currency(
			origin: OriginFor<T>,
			_currency_id: CurrencyId,
			call: Box<CallOf<T>>,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin.clone())?;
			call.dispatch(origin)
		}

		/// Fee paid by other account
		#[pallet::weight({
			let dispatch_info = call.get_dispatch_info();
			(T::WeightInfo::with_fee_paid_by().saturating_add(dispatch_info.weight), dispatch_info.class,)
		})]
		pub fn with_fee_paid_by(
			origin: OriginFor<T>,
			call: Box<CallOf<T>>,
			_payer_addr: T::AccountId,
			_payer_sig: MultiSignature,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin.clone())?;
			call.dispatch(origin)
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
	) -> FeeDetails<PalletBalanceOf<T>> {
		let dispatch_info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(&unchecked_extrinsic);
		Self::compute_fee_details(len, &dispatch_info, 0u32.into())
	}

	/// Compute the fee details for a particular transaction.
	pub fn compute_fee_details(
		len: u32,
		info: &DispatchInfoOf<CallOf<T>>,
		tip: PalletBalanceOf<T>,
	) -> FeeDetails<PalletBalanceOf<T>> {
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
	pub fn compute_fee(len: u32, info: &DispatchInfoOf<CallOf<T>>, tip: PalletBalanceOf<T>) -> PalletBalanceOf<T> {
		Self::compute_fee_details(len, info, tip).final_fee()
	}

	/// Compute the actual post dispatch fee details for a particular
	/// transaction.
	pub fn compute_actual_fee_details(
		len: u32,
		info: &DispatchInfoOf<CallOf<T>>,
		post_info: &PostDispatchInfoOf<CallOf<T>>,
		tip: PalletBalanceOf<T>,
	) -> FeeDetails<PalletBalanceOf<T>> {
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
		info: &DispatchInfoOf<CallOf<T>>,
		post_info: &PostDispatchInfoOf<CallOf<T>>,
		tip: PalletBalanceOf<T>,
	) -> PalletBalanceOf<T> {
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

	pub fn weight_to_fee(weight: Weight) -> PalletBalanceOf<T> {
		// cap the weight to the maximum defined in runtime, otherwise it will be the
		// `Bounded` maximum of its data type, which is not desired.
		let capped_weight = weight.min(T::BlockWeights::get().max_block);
		T::WeightToFee::weight_to_fee(&capped_weight)
	}

	/// If native asset is enough, return `None`, else return the fee amount should be swapped.
	fn check_native_is_not_enough(
		who: &T::AccountId,
		fee: PalletBalanceOf<T>,
		reason: WithdrawReasons,
	) -> Option<Balance> {
		let native_existential_deposit = <T as Config>::Currency::minimum_balance();
		let total_native = <T as Config>::Currency::free_balance(who);

		if fee.saturating_add(native_existential_deposit) <= total_native {
			// User's locked balance can't be transferable, which means can't be used for fee payment.
			if let Some(new_free_balance) = total_native.checked_sub(fee) {
				if T::Currency::ensure_can_withdraw(who, fee, reason, new_free_balance).is_ok() {
					return None;
				}
			}
			Some(fee)
		} else {
			Some(fee.saturating_add(native_existential_deposit.saturating_sub(total_native)))
		}
	}

	/// Determine the fee and surplus that should be withdraw from user. There are three kind call:
	/// - TransactionPayment::with_fee_path: swap with dex
	/// - TransactionPayment::with_fee_currency: swap with tx fee pool
	/// - others call: first use native asset, if not enough use alternative, or else use default.
	fn ensure_can_charge_fee_with_call(
		who: &T::AccountId,
		fee: PalletBalanceOf<T>,
		call: &CallOf<T>,
		reason: WithdrawReasons,
	) -> Result<(T::AccountId, Balance), DispatchError> {
		match call.is_sub_type() {
			Some(Call::with_fee_path { fee_swap_path, .. }) => {
				ensure!(
					fee_swap_path.len() > 1
						&& fee_swap_path.first() != Some(&T::NativeCurrencyId::get())
						&& fee_swap_path.last() == Some(&T::NativeCurrencyId::get()),
					Error::<T>::InvalidSwapPath
				);
				let fee = Self::check_native_is_not_enough(who, fee, reason).map_or_else(|| fee, |amount| amount);
				let custom_fee_surplus = T::CustomFeeSurplus::get().mul_ceil(fee);
				let custom_fee_amount = fee.saturating_add(custom_fee_surplus);
				T::DEX::swap_with_specific_path(
					who,
					fee_swap_path,
					SwapLimit::ExactTarget(Balance::MAX, custom_fee_amount),
				)
				.map(|_| (who.clone(), custom_fee_surplus))
			}
			Some(Call::with_fee_currency { currency_id, .. }) => {
				ensure!(
					TokenExchangeRate::<T>::contains_key(currency_id),
					Error::<T>::InvalidToken
				);
				let fee = Self::check_native_is_not_enough(who, fee, reason).map_or_else(|| fee, |amount| amount);
				let alternative_fee_surplus = T::AlternativeFeeSurplus::get().mul_ceil(fee);
				let custom_fee_surplus = T::CustomFeeSurplus::get().mul_ceil(fee);
				let (fee_amount, fee_surplus) = if T::DefaultFeeTokens::get().contains(currency_id) {
					(fee.saturating_add(alternative_fee_surplus), alternative_fee_surplus)
				} else {
					(fee.saturating_add(custom_fee_surplus), custom_fee_surplus)
				};
				Self::swap_from_pool_or_dex(who, fee_amount, *currency_id).map(|_| (who.clone(), fee_surplus))
			}
			Some(Call::with_fee_paid_by {
				call: _,
				payer_addr,
				payer_sig: _,
			}) => {
				// validate payer signature in runtime side, because `SignedExtension` between different runtime
				// may be different.
				Self::native_then_alternative_or_default(payer_addr, fee, WithdrawReasons::TRANSACTION_PAYMENT)
					.map(|surplus| (payer_addr.clone(), surplus))
			}
			_ => Self::native_then_alternative_or_default(who, fee, reason).map(|surplus| (who.clone(), surplus)),
		}
	}

	/// If native is enough, do nothing, return `Ok(0)` means there are none extra surplus fee.
	/// If native is not enough, try swap from tx fee pool or dex. As user can set his own
	/// `AlternativeFeeSwapPath`, this will direct swap from dex. Sometimes, user setting of
	/// `AlternativeFeeSwapPath` may be wrong or dex is not available, or user do not set any
	/// `AlternativeFeeSwapPath`, then use the `DefaultFeeTokens` to swap from tx fee pool.
	fn native_then_alternative_or_default(
		who: &T::AccountId,
		fee: PalletBalanceOf<T>,
		reason: WithdrawReasons,
	) -> Result<Balance, DispatchError> {
		if let Some(amount) = Self::check_native_is_not_enough(who, fee, reason) {
			// native asset is not enough
			let fee_surplus = T::AlternativeFeeSurplus::get().mul_ceil(fee);
			let fee_amount = fee_surplus.saturating_add(amount);

			// alter native fee swap path, swap from dex: O(1)
			if let Some(path) = AlternativeFeeSwapPath::<T>::get(who) {
				if T::DEX::swap_with_specific_path(who, &path, SwapLimit::ExactTarget(Balance::MAX, fee_amount)).is_ok()
				{
					return Ok(fee_surplus);
				}
			}

			// default fee tokens, swap from tx fee pool: O(1)
			for supply_currency_id in T::DefaultFeeTokens::get() {
				if Self::swap_from_pool_or_dex(who, fee_amount, supply_currency_id).is_ok() {
					return Ok(fee_surplus);
				}
			}

			// migration of `GlobalFeeSwapPath`. after Dapp using `with_fee_currency`, we can delete this.
			let global_fee_swap_path = GlobalFeeSwapPath::<T>::iter_values()
				.map(|v| v.into_inner())
				.collect::<Vec<_>>();
			for path in global_fee_swap_path {
				if let Some(supply_currency_id) = path.first() {
					if Self::swap_from_pool_or_dex(who, fee_amount, *supply_currency_id).is_ok() {
						return Ok(fee_surplus);
					}
				}
			}

			Err(DispatchError::Other("charge fee failed!"))
		} else {
			// native asset is enough
			Ok(0)
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
		let threshold_balance = SwapBalanceThreshold::<T>::get(supply_currency_id);
		if native_balance < threshold_balance {
			if let Some(trading_path) = GlobalFeeSwapPath::<T>::get(supply_currency_id) {
				let supply_balance = T::MultiCurrency::free_balance(supply_currency_id, &sub_account);
				let supply_amount =
					supply_balance.saturating_sub(T::MultiCurrency::minimum_balance(supply_currency_id));
				if let Ok((supply_amount, swap_native_balance)) = T::DEX::swap_with_specific_path(
					&sub_account,
					&trading_path,
					SwapLimit::ExactSupply(supply_amount, 0),
				) {
					// calculate and update new rate, also update the pool size
					let swap_exchange_rate = Ratio::saturating_from_rational(supply_amount, swap_native_balance);
					let new_pool_size = swap_native_balance.saturating_add(native_balance);
					let new_exchange_rate = Self::calculate_exchange_rate(supply_currency_id, swap_exchange_rate)?;

					TokenExchangeRate::<T>::insert(supply_currency_id, new_exchange_rate);
					PoolSize::<T>::insert(supply_currency_id, new_pool_size);
					Pallet::<T>::deposit_event(Event::<T>::ChargeFeePoolSwapped {
						sub_account: sub_account.clone(),
						supply_currency_id,
						old_exchange_rate: rate,
						swap_exchange_rate,
						new_exchange_rate,
						new_pool_size,
					});
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

	/// The sub account derivated by `PalletId`.
	fn sub_account_id(id: CurrencyId) -> T::AccountId {
		T::PalletId::get().into_sub_account_truncating(id)
	}

	/// Calculate the new exchange rate.
	/// old_rate * (threshold/poolSize) + swap_exchange_rate * (1-threshold/poolSize)
	fn calculate_exchange_rate(currency_id: CurrencyId, swap_exchange_rate: Ratio) -> Result<Ratio, Error<T>> {
		let threshold_balance = SwapBalanceThreshold::<T>::get(currency_id);
		let old_threshold_rate = Ratio::saturating_from_rational(threshold_balance, PoolSize::<T>::get(currency_id));
		let new_threshold_rate = Ratio::one().saturating_sub(old_threshold_rate);

		let rate = TokenExchangeRate::<T>::get(currency_id).ok_or(Error::<T>::InvalidRate)?;
		let old_parts = rate.saturating_mul(old_threshold_rate);
		let new_parts = swap_exchange_rate.saturating_mul(new_threshold_rate);
		let new_exchange_rate = old_parts.saturating_add(new_parts);
		Ok(new_exchange_rate)
	}

	/// Initiate a charge fee pool, transfer token from treasury account to sub account.
	pub fn initialize_pool(
		currency_id: CurrencyId,
		fee_swap_path: Vec<CurrencyId>,
		pool_size: Balance,
		swap_threshold: Balance,
	) -> DispatchResult {
		// first add to GlobalFeeSwapPath mapping storage
		ensure!(
			fee_swap_path.len() > 1
				&& fee_swap_path.first() != Some(&T::NativeCurrencyId::get())
				&& fee_swap_path.last() == Some(&T::NativeCurrencyId::get()),
			Error::<T>::InvalidSwapPath
		);
		let first_currency = *fee_swap_path.get(0).expect("ensured path not empty; qed");
		ensure!(currency_id == first_currency, Error::<T>::InvalidSwapPath);
		let global_mut = GlobalFeeSwapPath::<T>::try_mutate(currency_id, |maybe_path| -> DispatchResult {
			let path: BoundedVec<CurrencyId, T::TradingPathLimit> = fee_swap_path
				.clone()
				.try_into()
				.map_err(|_| Error::<T>::InvalidSwapPath)?;
			*maybe_path = Some(path);
			Ok(())
		});
		ensure!(global_mut.is_ok(), Error::<T>::InvalidSwapPath);

		// do tx fee pool pre-check
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

		// make sure trading path is valid, and the trading path is valid when swap from dex
		let (supply_amount, _) = T::DEX::get_swap_amount(
			&fee_swap_path,
			SwapLimit::ExactTarget(Balance::MAX, native_existential_deposit),
		)
		.ok_or(Error::<T>::DexNotAvailable)?;
		let exchange_rate = Ratio::saturating_from_rational(supply_amount, native_existential_deposit);

		// transfer initial tokens between treasury account and sub account of this enabled token
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

		// other storage
		SwapBalanceThreshold::<T>::insert(currency_id, swap_threshold);
		TokenExchangeRate::<T>::insert(currency_id, exchange_rate);
		PoolSize::<T>::insert(currency_id, pool_size);

		Self::deposit_event(Event::ChargeFeePoolEnabled {
			sub_account,
			currency_id,
			fee_swap_path,
			exchange_rate,
			pool_size,
			swap_threshold,
		});
		Ok(())
	}

	/// Disable a charge fee pool, transfer token from sub account back to treasury account.
	pub fn disable_pool(currency_id: CurrencyId) -> DispatchResult {
		ensure!(
			TokenExchangeRate::<T>::contains_key(currency_id),
			Error::<T>::InvalidToken
		);
		let treasury_account = T::TreasuryAccount::get();
		let sub_account = Self::sub_account_id(currency_id);
		let foreign_amount: Balance = T::MultiCurrency::free_balance(currency_id, &sub_account);
		let native_amount: Balance = T::Currency::free_balance(&sub_account);

		T::MultiCurrency::transfer(currency_id, &sub_account, &treasury_account, foreign_amount)?;
		T::Currency::transfer(
			&sub_account,
			&treasury_account,
			native_amount,
			ExistenceRequirement::AllowDeath,
		)?;

		TokenExchangeRate::<T>::remove(currency_id);
		PoolSize::<T>::remove(currency_id);
		SwapBalanceThreshold::<T>::remove(currency_id);
		GlobalFeeSwapPath::<T>::remove(currency_id);

		Self::deposit_event(Event::ChargeFeePoolDisabled {
			currency_id,
			foreign_amount,
			native_amount,
		});
		Ok(())
	}
}

/// Calculate the exchange rate of token in transaction fee pool.
pub struct BuyWeightRateOfTransactionFeePool<T, C>(sp_std::marker::PhantomData<(T, C)>);
impl<T: Config, C> BuyWeightRate for BuyWeightRateOfTransactionFeePool<T, C>
where
	C: Convert<MultiLocation, Option<CurrencyId>>,
{
	fn calculate_rate(multi_location: MultiLocation) -> Option<Ratio> {
		C::convert(multi_location).and_then(TokenExchangeRate::<T>::get)
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
	PalletBalanceOf<T>: Send + Sync + FixedPointOperand,
{
	/// utility constructor. Used only in client/factory code.
	pub fn from(fee: PalletBalanceOf<T>) -> Self {
		Self(fee)
	}

	fn withdraw_fee(
		&self,
		who: &T::AccountId,
		call: &CallOf<T>,
		info: &DispatchInfoOf<CallOf<T>>,
		len: usize,
	) -> Result<
		(
			PalletBalanceOf<T>,
			Option<NegativeImbalanceOf<T>>,
			Option<PalletBalanceOf<T>>,
			T::AccountId,
		),
		TransactionValidityError,
	> {
		let tip = self.0;
		let fee = Pallet::<T>::compute_fee(len as u32, info, tip);

		// Only mess with balances if fee is not zero.
		if fee.is_zero() {
			return Ok((fee, None, None, who.clone()));
		}

		let reason = if tip.is_zero() {
			WithdrawReasons::TRANSACTION_PAYMENT
		} else {
			WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
		};

		let (payer, fee_surplus) = Pallet::<T>::ensure_can_charge_fee_with_call(who, fee, call, reason)
			.map_err(|_| InvalidTransaction::Payment)?;

		// withdraw native currency as fee, also consider surplus when swap from dex or pool.
		match <T as Config>::Currency::withdraw(&payer, fee + fee_surplus, reason, ExistenceRequirement::KeepAlive) {
			Ok(imbalance) => Ok((fee + fee_surplus, Some(imbalance), Some(fee_surplus), payer)),
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	/// Get an appropriate priority for a transaction with the given `DispatchInfo`, encoded length
	/// and user-included tip.
	///
	/// The priority is based on the amount of `tip` the user is willing to pay per unit of either
	/// `weight` or `length`, depending which one is more limiting. For `Operational` extrinsics
	/// we add a "virtual tip" to the calculations.
	///
	/// The formula should simply be `tip / bounded_{weight|length}`, but since we are using
	/// integer division, we have no guarantees it's going to give results in any reasonable
	/// range (might simply end up being zero). Hence we use a scaling factor:
	/// `tip * (max_block_{weight|length} / bounded_{weight|length})`, since given current
	/// state of-the-art blockchains, number of per-block transactions is expected to be in a
	/// range reasonable enough to not saturate the `Balance` type while multiplying by the tip.
	fn get_priority(
		info: &DispatchInfoOf<CallOf<T>>,
		len: usize,
		tip: PalletBalanceOf<T>,
		final_fee: PalletBalanceOf<T>,
	) -> TransactionPriority {
		// Calculate how many such extrinsics we could fit into an empty block and take
		// the limiting factor.
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
{
	const IDENTIFIER: &'static str = "ChargeTransactionPayment";
	type AccountId = T::AccountId;
	type Call = CallOf<T>;
	type AdditionalSigned = ();
	type Pre = (
		PalletBalanceOf<T>,
		Self::AccountId,
		Option<NegativeImbalanceOf<T>>,
		PalletBalanceOf<T>,         // fee includes surplus
		Option<PalletBalanceOf<T>>, // surplus
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
		let (final_fee, _, _, _) = self.withdraw_fee(who, call, info, len)?;
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
		let (fee, imbalance, surplus, payer) = self.withdraw_fee(who, call, info, len)?;
		Ok((self.0, payer, imbalance, fee, surplus))
	}

	fn post_dispatch(
		pre: Option<Self::Pre>,
		info: &DispatchInfoOf<Self::Call>,
		post_info: &PostDispatchInfoOf<Self::Call>,
		len: usize,
		_result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		if let Some((tip, who, Some(payed), fee, surplus)) = pre {
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
			// the refund surplus also need to return back to user
			if let Some(surplus) = surplus {
				let percent = Percent::from_rational(surplus, fee.saturating_sub(surplus));
				let actual_surplus = percent.mul_ceil(actual_fee);
				refund = refund.saturating_sub(actual_surplus);
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
	fn reserve_fee(
		who: &T::AccountId,
		fee: PalletBalanceOf<T>,
		named: Option<ReserveIdentifier>,
	) -> Result<PalletBalanceOf<T>, DispatchError> {
		Pallet::<T>::native_then_alternative_or_default(who, fee, WithdrawReasons::TRANSACTION_PAYMENT)?;
		T::Currency::reserve_named(&named.unwrap_or(RESERVE_ID), who, fee)?;
		Ok(fee)
	}

	fn unreserve_fee(
		who: &T::AccountId,
		fee: PalletBalanceOf<T>,
		named: Option<ReserveIdentifier>,
	) -> PalletBalanceOf<T> {
		<T as Config>::Currency::unreserve_named(&named.unwrap_or(RESERVE_ID), who, fee)
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

	fn weight_to_fee(weight: Weight) -> PalletBalanceOf<T> {
		Pallet::<T>::weight_to_fee(weight)
	}

	/// Apply multiplier to fee, return the final fee. If multiplier is `None`, use
	/// `next_fee_multiplier`.
	fn apply_multiplier_to_fee(fee: PalletBalanceOf<T>, multiplier: Option<Multiplier>) -> PalletBalanceOf<T> {
		let multiplier = multiplier.unwrap_or_else(|| Pallet::<T>::next_fee_multiplier());
		multiplier.saturating_mul_int(fee)
	}
}
