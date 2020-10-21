//! # Accounts Module
//!
//! ## Overview
//!
//! Accounts module is responsible for opening and closing accounts in Acala,
//! and charge fee and tip in different currencies

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_module, decl_storage,
	dispatch::{DispatchResult, Dispatchable},
	ensure,
	traits::{
		Currency, ExistenceRequirement, Get, Happened, Imbalance, OnKilledAccount, OnUnbalanced, StoredMap,
		WithdrawReason,
	},
	weights::{
		DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo, Weight, WeightToFeeCoefficient, WeightToFeePolynomial,
	},
	IsSubType, StorageMap,
};
use frame_system::{self as system, ensure_signed, AccountInfo};
use orml_traits::{MultiCurrency, MultiLockableCurrency, MultiReservableCurrency, OnReceived};
use orml_utilities::with_transaction_result;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{
		AccountIdConversion, CheckedSub, Convert, DispatchInfoOf, PostDispatchInfoOf, SaturatedConversion, Saturating,
		SignedExtension, UniqueSaturatedInto, Zero,
	},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	FixedPointNumber, FixedPointOperand, FixedU128, ModuleId, Perquintill,
};
use sp_std::convert::Infallible;
use sp_std::{prelude::*, vec};
use support::{DEXManager, Ratio};

mod default_weight;
mod mock;
mod tests;

pub trait WeightInfo {
	fn close_account(c: u32) -> Weight;
	fn on_finalize() -> Weight;
}

/// Fee multiplier.
pub type Multiplier = FixedU128;

type PalletBalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::NegativeImbalance;

/// A struct to update the weight multiplier per block. It implements
/// `Convert<Multiplier, Multiplier>`, meaning that it can convert the previous
/// multiplier to the next one. This should be called on `on_finalize` of a
/// block, prior to potentially cleaning the weight data from the system module.
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
/// Where `(s', v)` must be given as the `Get` implementation of the `T` generic
/// type. Moreover, `M` must provide the minimum allowed value for the
/// multiplier. Note that a runtime should ensure with tests that the
/// combination of this `M` and `V` is not such that the multiplier can drop to
/// zero and never recover.
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
	T: frame_system::Trait,
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
	T: frame_system::Trait,
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

		// the computed ratio is only among the normal class.
		let normal_max_weight = <T as frame_system::Trait>::AvailableBlockRatio::get()
			* <T as frame_system::Trait>::MaximumBlockWeight::get();
		let normal_block_weight = <frame_system::Module<T>>::block_weight()
			.get(frame_support::weights::DispatchClass::Normal)
			.min(normal_max_weight);

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

pub trait Trait: system::Trait + orml_currencies::Trait {
	/// All non-native currency ids in Acala.
	type AllNonNativeCurrencyIds: Get<Vec<CurrencyId>>;

	/// Native currency id, the actual received currency type as fee for
	/// treasury. Should be ACA
	type NativeCurrencyId: Get<CurrencyId>;

	/// Stable currency id, should be AUSD
	type StableCurrencyId: Get<CurrencyId>;

	/// The currency type in which fees will be paid.
	type Currency: Currency<Self::AccountId> + Send + Sync;

	/// Currency to transfer, reserve/unreserve, lock/unlock assets
	type MultiCurrency: MultiLockableCurrency<Self::AccountId, Moment = Self::BlockNumber, CurrencyId = CurrencyId, Balance = Balance>
		+ MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// Handler for the unbalanced reduction when taking transaction fees. This
	/// is either one or two separate imbalances, the first is the transaction
	/// fee paid, the second is the tip paid, if any.
	type OnTransactionPayment: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// The fee to be paid for making a transaction; the per-byte portion.
	type TransactionByteFee: Get<PalletBalanceOf<Self>>;

	/// Convert a weight value into a deductible fee based on the currency type.
	type WeightToFee: WeightToFeePolynomial<Balance = PalletBalanceOf<Self>>;

	/// Update the multiplier of the next block, based on the previous block's
	/// weight.
	type FeeMultiplierUpdate: MultiplierUpdate;

	/// DEX to exchange currencies.
	type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

	/// Handler to trigger events when opening accounts

	/// Handler for the unbalanced reduction when taking transaction fees. This
	/// is either one or two separate imbalances, the first is the transaction
	/// fee paid, the second is the tip paid, if any.

	/// Event handler which calls when open account in system.
	type OnCreatedAccount: Happened<Self::AccountId>;

	/// Handler to kill account in system.
	type KillAccount: Happened<Self::AccountId>;

	/// Deposit for opening account, would be reserved until account closed.
	type NewAccountDeposit: Get<Balance>;

	/// The treasury module account id to recycle assets.
	type TreasuryModuleId: Get<ModuleId>;

	/// The max slippage allowed when swap open account deposit or fee with DEX
	type MaxSlippageSwapWithDEX: Get<Ratio>;

	/// Weight information for the extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_error! {
	/// Error for accounts manager module.
	pub enum Error for Module<T: Trait> {
		/// Balance is not sufficient
		NotEnoughBalance,
		/// Account ref count is not zero
		NonZeroRefCount,
		/// Account still has active reserved(include non-native token and native token beyond new account deposit)
		StillHasActiveReserved,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Accounts {
		pub NextFeeMultiplier get(fn next_fee_multiplier): Multiplier = Multiplier::saturating_from_integer(1);
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		/// All non-native currency ids in Acala.
		const AllNonNativeCurrencyIds: Vec<CurrencyId> = T::AllNonNativeCurrencyIds::get();

		/// Native currency id, the actual received currency type as fee for treasury.
		const NativeCurrencyId: CurrencyId = T::NativeCurrencyId::get();

		/// Stable currency id.
		const StableCurrencyId: CurrencyId = T::StableCurrencyId::get();

		/// Deposit for opening account, would be reserved until account closed.
		const NewAccountDeposit: Balance = T::NewAccountDeposit::get();

		/// The treasury module account id to recycle assets.
		const TreasuryModuleId: ModuleId = T::TreasuryModuleId::get();

		/// The max slippage allowed when swap open account deposit or fee with DEX
		const MaxSlippageSwapWithDEX: Ratio = T::MaxSlippageSwapWithDEX::get();

		/// The fee to be paid for making a transaction; the per-byte portion.
		const TransactionByteFee: PalletBalanceOf<T> = T::TransactionByteFee::get();

		/// The polynomial that is applied in order to derive fee from weight.
		const WeightToFee: Vec<WeightToFeeCoefficient<PalletBalanceOf<T>>> =
			T::WeightToFee::polynomial().to_vec();

		/// Kill self account from system.
		///
		/// The dispatch origin of this call must be Signed.
		///
		/// - `recipient`: the account as recipient to receive remaining currencies of the account will be killed,
		///					None means no recipient is specified.
		#[weight = <T as Trait>::WeightInfo::close_account(T::AllNonNativeCurrencyIds::get().len() as u32)]
		pub fn close_account(origin, recipient: Option<T::AccountId>) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;

				// check must allow death,
				// if native/non-native currencies has locks, means ref_count shouldn't be zero, can not close the account.
				ensure!(
					<system::Module<T>>::allow_death(&who),
					Error::<T>::NonZeroRefCount,
				);

				let native_currency_id = T::NativeCurrencyId::get();
				let new_account_deposit = T::NewAccountDeposit::get();
				let total_reserved_native = <T as Trait>::MultiCurrency::reserved_balance(native_currency_id, &who);

				// ensure total reserved native is lte new account deposit,
				// otherwise think the account still has active reserved kept by some bussiness.
				ensure!(
					new_account_deposit >= total_reserved_native,
					Error::<T>::StillHasActiveReserved,
				);
				let treasury_account = Self::treasury_account_id();
				let recipient = recipient.unwrap_or_else(|| treasury_account.clone());

				// unreserve all reserved native currency
				<T as Trait>::MultiCurrency::unreserve(native_currency_id, &who, total_reserved_native);

				// transfer all free to recipient
				<T as Trait>::MultiCurrency::transfer(native_currency_id, &who, &recipient, <T as Trait>::MultiCurrency::free_balance(native_currency_id, &who))?;

				// handle other non-native currencies
				for currency_id in T::AllNonNativeCurrencyIds::get() {
					// ensure the account has no active reserved of non-native token
					ensure!(
						<T as Trait>::MultiCurrency::reserved_balance(currency_id, &who).is_zero(),
						Error::<T>::StillHasActiveReserved,
					);

					// transfer all free to recipient
					<T as Trait>::MultiCurrency::transfer(currency_id, &who, &recipient, <T as Trait>::MultiCurrency::free_balance(currency_id, &who))?;
				}

				// finally kill the account
				T::KillAccount::happened(&who);

				Ok(())
			})?;
		}

		/// `on_initialize` to return the weight used in `on_finalize`.
		fn on_initialize(now: T::BlockNumber) -> Weight {
			<T as Trait>::WeightInfo::on_finalize()
		}

		fn on_finalize() {
			NextFeeMultiplier::mutate(|fm| {
				*fm = T::FeeMultiplierUpdate::convert(*fm);
			});
		}

		fn integrity_test() {
			// given weight == u64, we build multipliers from `diff` of two weight values, which can
			// at most be MaximumBlockWeight. Make sure that this can fit in a multiplier without
			// loss.
			use sp_std::convert::TryInto;
			assert!(
				<Multiplier as sp_runtime::traits::Bounded>::max_value() >=
				Multiplier::checked_from_integer(
					<T as frame_system::Trait>::MaximumBlockWeight::get().try_into().unwrap()
				).unwrap(),
			);

			// This is the minimum value of the multiplier. Make sure that if we collapse to this
			// value, we can recover with a reasonable amount of traffic. For this test we assert
			// that if we collapse to minimum, the trend will be positive with a weight value
			// which is 1% more than the target.
			let min_value = T::FeeMultiplierUpdate::min();
			let mut target =
				T::FeeMultiplierUpdate::target() *
				(T::AvailableBlockRatio::get() * T::MaximumBlockWeight::get());

			// add 1 percent;
			let addition = target / 100;
			if addition == 0 {
				// this is most likely because in a test setup we set everything to ().
				return;
			}
			target += addition;

			sp_io::TestExternalities::new_empty().execute_with(|| {
				<frame_system::Module<T>>::set_block_limits(target, 0);
				let next = T::FeeMultiplierUpdate::convert(min_value);
				assert!(next > min_value, "The minimum bound of the multiplier is too low. When \
					block saturation is more than target by 1% and multiplier is minimal then \
					the multiplier doesn't increase."
				);
			})
		}
	}
}

impl<T: Trait> Module<T> {
	/// Get treasury account id.
	pub fn treasury_account_id() -> T::AccountId {
		T::TreasuryModuleId::get().into_account()
	}

	/// Open account by reserve native token.
	///
	/// If not enough free balance to reserve, all the balance would be
	/// transferred to treasury instead.
	fn open_account(k: &T::AccountId) {
		let native_currency_id = T::NativeCurrencyId::get();
		if <T as Trait>::MultiCurrency::reserve(native_currency_id, k, T::NewAccountDeposit::get()).is_ok() {
			T::OnCreatedAccount::happened(&k);
		} else {
			let treasury_account = Self::treasury_account_id();

			// Note: will not reap treasury account even though it cannot reserve open
			// account deposit best practice is to ensure that the first transfer received
			// by treasury account is sufficient to open an account.
			if *k != treasury_account {
				// send dust native currency to treasury account.
				// transfer all free balances from a new account to treasury account, so it
				// shouldn't fail. but even it failed, leave some dust storage is not a critical
				// issue, just open account without reserve NewAccountDeposit.
				if <T as Trait>::MultiCurrency::transfer(
					native_currency_id,
					k,
					&treasury_account,
					<T as Trait>::MultiCurrency::free_balance(native_currency_id, k),
				)
				.is_ok()
				{
					// remove the account info pretend that opening account has never happened
					system::Account::<T>::remove(k);
				}
			}
		}
	}
}

/// Note: Currently `pallet_balances` does not implement `OnReceived`,
/// which means here only do the preparations for opening an account by
/// non-native currency, actual process of opening account is handled by
/// `StoredMap`.
impl<T: Trait> OnReceived<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn on_received(who: &T::AccountId, currency_id: CurrencyId, _: Balance) {
		let native_currency_id = T::NativeCurrencyId::get();

		if !<Self as StoredMap<_, _>>::is_explicit(who) && currency_id != native_currency_id {
			let stable_currency_id = T::StableCurrencyId::get();
			let trading_path = if currency_id == stable_currency_id {
				vec![stable_currency_id, native_currency_id]
			} else {
				vec![currency_id, stable_currency_id, native_currency_id]
			};

			// Successful swap will cause changes in native currency,
			// which also means that it will open a new account
			// exchange token to native currency and open account.
			// If swap failed, will leave some dust storage is not a critical issue,
			// just open account without reserve NewAccountDeposit.
			// Don't recycle non-native to avoid unreasonable loss
			// due to insufficient liquidity of DEX, can try to open this
			// account again later. If want to recycle dust non-native,
			// should handle by the currencies module.
			let _ = T::DEX::swap_with_exact_target(
				who,
				&trading_path,
				T::NewAccountDeposit::get(),
				<T as Trait>::MultiCurrency::free_balance(currency_id, who),
				Some(T::MaxSlippageSwapWithDEX::get()),
			);
		}
	}
}

/// Fork StoredMap in frame_system,  still use `Account` storage of
/// frame_system.
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

/// Split an `option` into two constituent options, as defined by a `splitter`
/// function.
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
	fn on_killed_account(_who: &T::AccountId) {}
}

impl<T: Trait> Module<T>
where
	PalletBalanceOf<T>: FixedPointOperand,
{
	/// Query the data that we know about the fee of a given `call`.
	///
	/// This module is not and cannot be aware of the internals of a signed
	/// extension, for example a tip. It only interprets the extrinsic as some
	/// encoded value and accounts for its weight and length, the runtime's
	/// extrinsic base weight, and the current fee multiplier.
	///
	/// All dispatchables must be annotated with weight and will have some fee
	/// info. This function always returns.
	pub fn query_info<Extrinsic: GetDispatchInfo>(
		unchecked_extrinsic: Extrinsic,
		len: u32,
	) -> RuntimeDispatchInfo<PalletBalanceOf<T>>
	where
		T: Send + Sync,
		PalletBalanceOf<T>: Send + Sync,
		T::Call: Dispatchable<Info = DispatchInfo>,
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

	/// Compute the final fee value for a particular transaction.
	///
	/// The final fee is composed of:
	///   - `base_fee`: This is the minimum amount a user pays for a
	///     transaction. It is declared as a base _weight_ in the runtime and
	///     converted to a fee using `WeightToFee`.
	///   - `len_fee`: The length fee, the amount paid for the encoded length
	///     (in bytes) of the transaction.
	///   - `weight_fee`: This amount is computed based on the weight of the
	///     transaction. Weight accounts for the execution time of a
	///     transaction.
	///   - `targeted_fee_adjustment`: This is a multiplier that can tune the
	///     final fee based on the congestion of the network.
	///   - (Optional) `tip`: If included in the transaction, the tip will be
	///     added on top. Only signed transactions can have a tip.
	///
	/// The base fee and adjusted weight and length fees constitute the
	/// _inclusion fee,_ which is the minimum fee for a transaction to be
	/// included in a block.
	///
	/// ```ignore
	/// inclusion_fee = base_fee + len_fee + [targeted_fee_adjustment * weight_fee];
	/// final_fee = inclusion_fee + tip;
	/// ```
	pub fn compute_fee(len: u32, info: &DispatchInfoOf<T::Call>, tip: PalletBalanceOf<T>) -> PalletBalanceOf<T>
	where
		T::Call: Dispatchable<Info = DispatchInfo>,
	{
		Self::compute_fee_raw(len, info.weight, tip, info.pays_fee)
	}

	/// Compute the actual post dispatch fee for a particular transaction.
	///
	/// Identical to `compute_fee` with the only difference that the post
	/// dispatch corrected weight is used for the weight fee calculation.
	pub fn compute_actual_fee(
		len: u32,
		info: &DispatchInfoOf<T::Call>,
		post_info: &PostDispatchInfoOf<T::Call>,
		tip: PalletBalanceOf<T>,
	) -> PalletBalanceOf<T>
	where
		T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	{
		Self::compute_fee_raw(len, post_info.calc_actual_weight(info), tip, post_info.pays_fee(info))
	}

	fn compute_fee_raw(len: u32, weight: Weight, tip: PalletBalanceOf<T>, pays_fee: Pays) -> PalletBalanceOf<T> {
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

			let base_fee = Self::weight_to_fee(T::ExtrinsicBaseWeight::get());
			base_fee
				.saturating_add(fixed_len_fee)
				.saturating_add(adjusted_weight_fee)
				.saturating_add(tip)
		} else {
			tip
		}
	}

	fn weight_to_fee(weight: Weight) -> PalletBalanceOf<T> {
		// cap the weight to the maximum defined in runtime, otherwise it will be the
		// `Bounded` maximum of its data type, which is not desired.
		let capped_weight = weight.min(<T as frame_system::Trait>::MaximumBlockWeight::get());
		T::WeightToFee::calc(&capped_weight)
	}
}

impl<T> Convert<Weight, PalletBalanceOf<T>> for Module<T>
where
	T: Trait,
	PalletBalanceOf<T>: FixedPointOperand,
{
	/// Compute the fee for the specified weight.
	///
	/// This fee is already adjusted by the per block fee adjustment factor and
	/// is therefore the share that the weight contributes to the overall fee of
	/// a transaction. It is mainly for informational purposes and not used in
	/// the actual fee calculation.
	fn convert(weight: Weight) -> PalletBalanceOf<T> {
		NextFeeMultiplier::get().saturating_mul_int(Self::weight_to_fee(weight))
	}
}

/// Require the transactor pay for themselves and maybe include a tip to gain
/// additional priority in the queue.
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
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<orml_currencies::Call<T>>,
	PalletBalanceOf<T>: Send + Sync + FixedPointOperand,
{
	/// utility constructor. Used only in client/factory code.
	pub fn from(fee: PalletBalanceOf<T>) -> Self {
		Self(fee)
	}

	fn withdraw_fee(
		&self,
		who: &T::AccountId,
		_call: &T::Call,
		info: &DispatchInfoOf<T::Call>,
		len: usize,
	) -> Result<(PalletBalanceOf<T>, Option<NegativeImbalanceOf<T>>), TransactionValidityError> {
		// pay any fees.
		let tip = self.0;
		let fee = Module::<T>::compute_fee(len as u32, info, tip);

		let reason = if tip.is_zero() {
			WithdrawReason::TransactionPayment.into()
		} else {
			WithdrawReason::TransactionPayment | WithdrawReason::Tip
		};

		// check native balance if is enough
		let native_is_enough = <T as Trait>::Currency::free_balance(who)
			.checked_sub(&fee)
			.map_or(false, |new_free_balance| {
				<T as Trait>::Currency::ensure_can_withdraw(who, fee, reason, new_free_balance).is_ok()
			});

		// try to use non-native currency to swap native currency by exchange with DEX
		if !native_is_enough {
			let native_currency_id = T::NativeCurrencyId::get();
			let stable_currency_id = T::StableCurrencyId::get();
			let other_currency_ids = T::AllNonNativeCurrencyIds::get();
			let price_impact_limit = Some(T::MaxSlippageSwapWithDEX::get());
			// Note: in fact, just obtain the gap between of fee and usable native currency
			// amount, but `Currency` does not expose interface to get usable balance by
			// specific reason. Here try to swap the whole fee by non-native currency.
			let balance_fee: Balance = fee.unique_saturated_into();

			// iterator non-native currencies to get enough fee
			for currency_id in other_currency_ids {
				let trading_path = if currency_id == stable_currency_id {
					vec![stable_currency_id, native_currency_id]
				} else {
					vec![currency_id, stable_currency_id, native_currency_id]
				};

				if T::DEX::swap_with_exact_target(
					who,
					&trading_path,
					balance_fee,
					<T as Trait>::MultiCurrency::free_balance(currency_id, who),
					price_impact_limit,
				)
				.is_ok()
				{
					// successfully swap, break iteration
					break;
				}
			}
		}

		// withdraw native currency as fee
		match <T as Trait>::Currency::withdraw(who, fee, reason, ExistenceRequirement::KeepAlive) {
			Ok(imbalance) => Ok((fee, Some(imbalance))),
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	/// Get an appropriate priority for a transaction with the given length and
	/// info.
	///
	/// This will try and optimise the `fee/weight` `fee/length`, whichever is
	/// consuming more of the maximum corresponding limit.
	///
	/// For example, if a transaction consumed 1/4th of the block length and
	/// half of the weight, its final priority is `fee * min(2, 4) = fee * 2`.
	/// If it consumed `1/4th` of the block length and the entire block weight
	/// `(1/1)`, its priority is `fee * min(1, 4) = fee * 1`. This means
	///  that the transaction which consumes more resources (either length or
	/// weight) with the same `fee` ends up having lower priority.
	fn get_priority(len: usize, info: &DispatchInfoOf<T::Call>, final_fee: PalletBalanceOf<T>) -> TransactionPriority {
		let weight_saturation = T::MaximumBlockWeight::get() / info.weight.max(1);
		let len_saturation = T::MaximumBlockLength::get() as u64 / (len as u64).max(1);
		let coefficient: PalletBalanceOf<T> = weight_saturation
			.min(len_saturation)
			.saturated_into::<PalletBalanceOf<T>>();
		final_fee
			.saturating_mul(coefficient)
			.saturated_into::<TransactionPriority>()
	}
}

impl<T: Trait + Send + Sync> SignedExtension for ChargeTransactionPayment<T>
where
	PalletBalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand,
	T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<orml_currencies::Call<T>>,
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
			let actual_fee = Module::<T>::compute_actual_fee(len as u32, info, post_info, tip);
			let refund = fee.saturating_sub(actual_fee);
			let actual_payment = match <T as Trait>::Currency::deposit_into_existing(&who, refund) {
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

			// distribute fee
			<T as Trait>::OnTransactionPayment::on_unbalanceds(
				Some(imbalances.0).into_iter().chain(Some(imbalances.1)),
			);
		}
		Ok(())
	}
}
