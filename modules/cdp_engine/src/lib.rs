#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{debug, decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, IsSubType};
use orml_traits::{arithmetic::Signed, MultiCurrency, MultiCurrencyExtended};
use rstd::{convert::TryInto, marker, prelude::*};
use sp_runtime::{
	offchain::{storage::StorageValueRef, Duration, Timestamp},
	traits::{BlakeTwo256, CheckedAdd, CheckedSub, Convert, EnsureOrigin, Hash, Saturating, UniqueSaturatedInto, Zero},
	transaction_validity::{InvalidTransaction, TransactionPriority, TransactionValidity, ValidTransaction},
	DispatchResult, RandomNumberGenerator, RuntimeDebug,
};
use support::{
	CDPTreasury, CDPTreasuryExtended, DexManager, EmergencyShutdown, ExchangeRate, Price, PriceProvider, Rate, Ratio,
	RiskManager,
};
use system::{ensure_none, ensure_root, offchain::SubmitUnsignedTransaction};

mod debit_exchange_rate_convertor;
pub use debit_exchange_rate_convertor::DebitExchangeRateConvertor;

mod mock;
mod tests;

const LOCK_EXPIRE_DURATION: u64 = 300_000; // 5 min
const LOCK_UPDATE_DURATION: u64 = 240_000; // 4 min
const DB_PREFIX: &[u8] = b"acala/cdp-engine-offchain-worker/";

type CurrencyIdOf<T> = <<T as loans::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as loans::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AmountOf<T> = <<T as loans::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

pub trait Trait: system::Trait + loans::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type PriceSource: PriceProvider<CurrencyIdOf<Self>, Price>;
	type CollateralCurrencyIds: Get<Vec<CurrencyIdOf<Self>>>;
	type GlobalStabilityFee: Get<Rate>;
	type DefaultLiquidationRatio: Get<Ratio>;
	type DefaultDebitExchangeRate: Get<ExchangeRate>;
	type DefaultLiquidationPenalty: Get<Rate>;
	type MinimumDebitValue: Get<BalanceOf<Self>>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
	type Treasury: CDPTreasuryExtended<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
	type UpdateOrigin: EnsureOrigin<Self::Origin>;
	type MaxSlippageSwapWithDex: Get<Ratio>;
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyIdOf<Self>, Balance = BalanceOf<Self>>;
	type Dex: DexManager<Self::AccountId, CurrencyIdOf<Self>, BalanceOf<Self>>;

	/// A dispatchable call type.
	type Call: From<Call<Self>> + IsSubType<Module<Self>, Self>;

	/// A transaction submitter.
	type SubmitTransaction: SubmitUnsignedTransaction<Self, <Self as Trait>::Call>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyIdOf<T>,
		Balance = BalanceOf<T>,
	{
		LiquidateUnsafeCdp(CurrencyId, AccountId, Balance, Balance),
		SettleCdpInDebit(CurrencyId, AccountId),
		UpdateStabilityFee(CurrencyId, Option<Rate>),
		UpdateLiquidationRatio(CurrencyId, Option<Ratio>),
		UpdateLiquidationPenalty(CurrencyId, Option<Rate>),
		UpdateRequiredCollateralRatio(CurrencyId, Option<Ratio>),
		UpdateMaximumTotalDebitValue(CurrencyId, Balance),
	}
);

decl_error! {
	/// Error for cdp engine module.
	pub enum Error for Module<T: Trait> {
		ExceedDebitValueHardCap,
		UpdatePositionFailed,
		DebitAmountConvertFailed,
		AmountConvertFailed,
		BelowRequiredCollateralRatio,
		BelowLiquidationRatio,
		CollateralRatioStillSafe,
		NotValidCurrencyId,
		RemainDebitValueTooSmall,
		GrabCollateralAndDebitFailed,
		BalanceOverflow,
		InvalidFeedPrice,
		AlreadyNoDebit,
		AlreadyShutdown,
		NoDebitInCdp,
		MustAfterShutdown,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as CdpEngine {
		pub StabilityFee get(fn stability_fee): map hasher(twox_64_concat) CurrencyIdOf<T> => Option<Rate>;
		pub LiquidationRatio get(fn liquidation_ratio): map hasher(twox_64_concat) CurrencyIdOf<T> => Option<Ratio>;
		pub LiquidationPenalty get(fn liquidation_penalty): map hasher(twox_64_concat) CurrencyIdOf<T> => Option<Rate>;
		pub RequiredCollateralRatio get(fn required_collateral_ratio): map hasher(twox_64_concat) CurrencyIdOf<T> => Option<Ratio>;
		pub MaximumTotalDebitValue get(fn maximum_total_debit_value): map hasher(twox_64_concat) CurrencyIdOf<T> => BalanceOf<T>;
		pub DebitExchangeRate get(fn debit_exchange_rate): map hasher(twox_64_concat) CurrencyIdOf<T> => Option<ExchangeRate>;
		pub IsShutdown get(fn is_shutdown): bool;
	}

	add_extra_genesis {
		config(collaterals_params): Vec<(CurrencyIdOf<T>, Option<Rate>, Option<Ratio>, Option<Rate>, Option<Ratio>, BalanceOf<T>)>;
		build(|config: &GenesisConfig<T>| {
			config.collaterals_params.iter().for_each(|(
				currency_id,
				stability_fee,
				liquidation_ratio,
				liquidation_penalty,
				required_collateral_ratio,
				maximum_total_debit_value,
			)| {
				if let Some(val) = stability_fee {
					<StabilityFee<T>>::insert(currency_id, val);
				}
				if let Some(val) = liquidation_ratio {
					<LiquidationRatio<T>>::insert(currency_id, val);
				}
				if let Some(val) = liquidation_penalty {
					<LiquidationPenalty<T>>::insert(currency_id, val);
				}
				if let Some(val) = required_collateral_ratio {
					<RequiredCollateralRatio<T>>::insert(currency_id, val);
				}
				<MaximumTotalDebitValue<T>>::insert(currency_id, maximum_total_debit_value);
			});
		});
	}
}

/// Error which may occur while executing the off-chain code.
#[cfg_attr(test, derive(PartialEq))]
enum OffchainErr {
	FailedToAcquireLock,
	SubmitTransaction,
	NotValidator,
	LockStillInLocked,
}

// The lock to limit the number of offchain worker at the same time.
// Before expire timestamp, can not start new offchain worker
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
struct OffchainWorkerLock {
	pub previous_position: u32,
	pub expire_timestamp: Timestamp,
}

impl rstd::fmt::Debug for OffchainErr {
	fn fmt(&self, fmt: &mut rstd::fmt::Formatter) -> rstd::fmt::Result {
		match *self {
			OffchainErr::FailedToAcquireLock => write!(fmt, "Failed to acquire lock"),
			OffchainErr::SubmitTransaction => write!(fmt, "Failed to submit transaction"),
			OffchainErr::NotValidator => write!(fmt, "Not validator"),
			OffchainErr::LockStillInLocked => write!(fmt, "Liquidator lock is still in locked"),
		}
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const CollateralCurrencyIds: Vec<CurrencyIdOf<T>> = T::CollateralCurrencyIds::get();
		const GlobalStabilityFee: Rate = T::GlobalStabilityFee::get();
		const DefaultLiquidationRatio: Ratio = T::DefaultLiquidationRatio::get();
		const DefaultDebitExchangeRate: ExchangeRate = T::DefaultDebitExchangeRate::get();
		const MinimumDebitValue: BalanceOf<T> = T::MinimumDebitValue::get();
		const GetStableCurrencyId: CurrencyIdOf<T> = T::GetStableCurrencyId::get();
		const MaxSlippageSwapWithDex: Ratio = T::MaxSlippageSwapWithDex::get();
		const DefaultLiquidationPenalty: Rate = T::DefaultLiquidationPenalty::get();

		pub fn set_collateral_params(
			origin,
			currency_id: CurrencyIdOf<T>,
			stability_fee: Option<Option<Rate>>,
			liquidation_ratio: Option<Option<Ratio>>,
			liquidation_penalty: Option<Option<Rate>>,
			required_collateral_ratio: Option<Option<Ratio>>,
			maximum_total_debit_value: Option<BalanceOf<T>>,
		) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			if let Some(update) = stability_fee {
				if let Some(val) = update {
					<StabilityFee<T>>::insert(currency_id, val);
				} else {
					<StabilityFee<T>>::remove(currency_id);
				}
				Self::deposit_event(RawEvent::UpdateStabilityFee(currency_id, update));
			}
			if let Some(update) = liquidation_ratio {
				if let Some(val) = update {
					<LiquidationRatio<T>>::insert(currency_id, val);
				} else {
					<LiquidationRatio<T>>::remove(currency_id);
				}
				Self::deposit_event(RawEvent::UpdateLiquidationRatio(currency_id, update));
			}
			if let Some(update) = liquidation_penalty {
				if let Some(val) = update {
					<LiquidationPenalty<T>>::insert(currency_id, val);
				} else {
					<LiquidationPenalty<T>>::remove(currency_id);
				}
				Self::deposit_event(RawEvent::UpdateLiquidationPenalty(currency_id, update));
			}
			if let Some(update) = required_collateral_ratio {
				if let Some(val) = update {
					<RequiredCollateralRatio<T>>::insert(currency_id, val);
				} else {
					<RequiredCollateralRatio<T>>::remove(currency_id);
				}
				Self::deposit_event(RawEvent::UpdateRequiredCollateralRatio(currency_id, update));
			}
			if let Some(val) = maximum_total_debit_value {
				<MaximumTotalDebitValue<T>>::insert(currency_id, val);
				Self::deposit_event(RawEvent::UpdateMaximumTotalDebitValue(currency_id, val));
			}
		}

		fn on_finalize(_now: T::BlockNumber) {
			// collect stability fee for all types of collateral
			if !Self::is_shutdown() {
				let global_stability_fee = T::GlobalStabilityFee::get();

				for currency_id in T::CollateralCurrencyIds::get() {
					let debit_exchange_rate = Self::get_debit_exchange_rate(currency_id);
					let stability_fee_rate = Self::stability_fee(currency_id)
						.unwrap_or_default()
						.saturating_add(global_stability_fee);
					let total_debits = <loans::Module<T>>::total_debits(currency_id);
					if !stability_fee_rate.is_zero() && !total_debits.is_zero() {
						let debit_exchange_rate_increment = debit_exchange_rate.saturating_mul(stability_fee_rate);

						// update exchange rate
						let new_debit_exchange_rate = debit_exchange_rate.saturating_add(debit_exchange_rate_increment);
						<DebitExchangeRate<T>>::insert(currency_id, new_debit_exchange_rate);

						// issue stablecoin to surplus pool
						let total_debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, total_debits));
						let issued_stable_coin_balance = debit_exchange_rate_increment.saturating_mul_int(&total_debit_value);
						<T as Trait>::Treasury::on_system_surplus(issued_stable_coin_balance);
					}
				}
			}
		}

		// unsigned tx to liquidate unsafe cdp, submitted by offchain worker
		pub fn liquidate(
			origin,
			currency_id: CurrencyIdOf<T>,
			who: T::AccountId,
		) {
			ensure_none(origin)?;
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);
			Self::liquidate_unsafe_cdp(who, currency_id)?;
		}

		// unsigned tx to settle cdp which has debit, submitted by offchain worker
		pub fn settle(
			origin,
			currency_id: CurrencyIdOf<T>,
			who: T::AccountId,
		) {
			ensure_none(origin)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);
			Self::settle_cdp_has_debit(who, currency_id)?;
		}

		// Runs after every block.
		fn offchain_worker(now: T::BlockNumber) {
			if let Err(e) = Self::_offchain_worker(now) {
				debug::info!(
					target: "cdp-engine offchain worker",
					"cannot run offchain worker at {:?}: {:?}",
					now,
					e,
				);
			}
		}
	}
}

impl<T: Trait> Module<T> {
	fn acquire_offchain_worker_lock() -> Result<OffchainWorkerLock, OffchainErr> {
		let storage_key = DB_PREFIX.to_vec();
		let storage = StorageValueRef::persistent(&storage_key);
		let collateral_currency_ids = T::CollateralCurrencyIds::get();

		let acquire_lock = storage.mutate(|lock: Option<Option<OffchainWorkerLock>>| {
			match lock {
				None => {
					//start with random collateral
					let random_seed = runtime_io::offchain::random_seed();
					let mut rng = RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(&random_seed[..]));
					let expire_timestamp =
						runtime_io::offchain::timestamp().add(Duration::from_millis(LOCK_EXPIRE_DURATION));

					Ok(OffchainWorkerLock {
						previous_position: rng.pick_u32(collateral_currency_ids.len() as u32),
						expire_timestamp: expire_timestamp,
					})
				}
				Some(Some(lock)) if runtime_io::offchain::timestamp() >= lock.expire_timestamp => {
					let execute_position = if lock.previous_position < collateral_currency_ids.len() as u32 - 1 {
						lock.previous_position + 1
					} else {
						0
					};
					let expire_timestamp =
						runtime_io::offchain::timestamp().add(Duration::from_millis(LOCK_EXPIRE_DURATION));

					Ok(OffchainWorkerLock {
						previous_position: execute_position,
						expire_timestamp: expire_timestamp,
					})
				}
				_ => Err(OffchainErr::LockStillInLocked),
			}
		})?;

		acquire_lock.map_err(|_| OffchainErr::FailedToAcquireLock)
	}

	fn release_offchain_worker_lock(previous_position: u32) {
		let storage_key = DB_PREFIX.to_vec();
		let storage = StorageValueRef::persistent(&storage_key);

		if let Some(Some(lock)) = storage.get::<OffchainWorkerLock>() {
			if lock.previous_position == previous_position {
				storage.set(&OffchainWorkerLock {
					previous_position: previous_position,
					expire_timestamp: runtime_io::offchain::timestamp(),
				});
			}
		}
	}

	fn extend_offchain_worker_lock_if_needed() {
		let storage_key = DB_PREFIX.to_vec();
		let storage = StorageValueRef::persistent(&storage_key);

		if let Some(Some(lock)) = storage.get::<OffchainWorkerLock>() {
			if lock.expire_timestamp
				<= runtime_io::offchain::timestamp().add(Duration::from_millis(LOCK_UPDATE_DURATION))
			{
				storage.set(&OffchainWorkerLock {
					previous_position: lock.previous_position,
					expire_timestamp: runtime_io::offchain::timestamp()
						.add(Duration::from_millis(LOCK_EXPIRE_DURATION)),
				});
			}
		}
	}

	fn submit_unsigned_liquidation_tx(currency_id: CurrencyIdOf<T>, who: T::AccountId) -> Result<(), OffchainErr> {
		let call = Call::<T>::liquidate(currency_id, who);
		T::SubmitTransaction::submit_unsigned(call).map_err(|_| OffchainErr::SubmitTransaction)?;
		Ok(())
	}

	fn submit_unsigned_settle_tx(currency_id: CurrencyIdOf<T>, who: T::AccountId) -> Result<(), OffchainErr> {
		let call = Call::<T>::settle(currency_id, who);
		T::SubmitTransaction::submit_unsigned(call).map_err(|_| OffchainErr::SubmitTransaction)?;
		Ok(())
	}

	fn liquidate_specific_collateral(currency_id: CurrencyIdOf<T>) {
		for (_, key) in <loans::Module<T>>::debits_iterator_with_collateral_prefix(currency_id) {
			if let Some((_, account_id)) = key {
				if Self::is_unsafe_cdp(currency_id, &account_id) {
					if let Err(e) = Self::submit_unsigned_liquidation_tx(currency_id, account_id.clone()) {
						debug::warn!(
							target: "cdp-engine offchain worker",
							"submit unsigned liquidation tx for \nCDP - AccountId {:?} CurrencyId {:?} \nfailed : {:?}",
							account_id, currency_id, e,
						);
					} else {
						debug::debug!(
							target: "cdp-engine offchain worker",
							"successfully submit unsigned liquidation tx for \nCDP - AccountId {:?} CurrencyId {:?}",
							account_id, currency_id,
						);
					}
				}
			}

			// check the expire timestamp of lock that is needed to extend
			Self::extend_offchain_worker_lock_if_needed();
		}
	}

	fn settle_specific_collateral(currency_id: CurrencyIdOf<T>) {
		for (debit, key) in <loans::Module<T>>::debits_iterator_with_collateral_prefix(currency_id) {
			if let Some((_, account_id)) = key {
				if !debit.is_zero() {
					if let Err(e) = Self::submit_unsigned_settle_tx(currency_id, account_id.clone()) {
						debug::warn!(
							target: "cdp-engine offchain worker",
							"submit unsigned settlement tx for \nCDP - AccountId {:?} CurrencyId {:?} \nfailed : {:?}",
							account_id, currency_id, e,
						);
					} else {
						debug::debug!(
							target: "cdp-engine offchain worker",
							"successfully submit unsigned settlement tx for \nCDP - AccountId {:?} CurrencyId {:?}",
							account_id, currency_id,
						);
					}
				}
			}

			// check the expire timestamp of lock that is needed to extend
			Self::extend_offchain_worker_lock_if_needed();
		}
	}

	fn _offchain_worker(block_number: T::BlockNumber) -> Result<(), OffchainErr> {
		// check if we are a potential validator
		if !runtime_io::offchain::is_validator() {
			return Err(OffchainErr::NotValidator);
		}

		// Acquire offchain worker lock.
		// If succeeded, update the lock, otherwise return error
		let OffchainWorkerLock {
			previous_position,
			expire_timestamp: _,
		} = Self::acquire_offchain_worker_lock()?;

		// start
		let collateral_currency_ids = T::CollateralCurrencyIds::get();
		let currency_id = collateral_currency_ids[(previous_position as usize)];

		if !Self::is_shutdown() {
			debug::debug!(
				target: "cdp-engine offchain worker",
				"execute automatic liquidation at block: {:?} for collateral: {:?}",
				block_number,
				currency_id,
			);
			Self::liquidate_specific_collateral(currency_id);
		} else {
			debug::debug!(
				target: "cdp-engine offchain worker",
				"execute automatic settlement at block: {:?} for collateral: {:?}",
				block_number,
				currency_id,
			);
			Self::settle_specific_collateral(currency_id);
		}

		// finally, reset the expire timestamp to now in order to release lock in advance.
		Self::release_offchain_worker_lock(previous_position);
		debug::debug!(
			target: "cdp-engine offchain worker",
			"offchain worker start at block: {:?} already done!",
			block_number,
		);

		Ok(())
	}

	pub fn is_unsafe_cdp(currency_id: CurrencyIdOf<T>, who: &T::AccountId) -> bool {
		let debit_balance = <loans::Module<T>>::debits(currency_id, who).0;
		let collateral_balance = <loans::Module<T>>::collaterals(who, currency_id);
		let stable_currency_id = T::GetStableCurrencyId::get();

		if debit_balance.is_zero() {
			false
		} else if let Some(feed_price) = T::PriceSource::get_price(stable_currency_id, currency_id) {
			let collateral_ratio =
				Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);
			let liquidation_ratio = Self::get_liquidation_ratio(currency_id);
			collateral_ratio < liquidation_ratio
		} else {
			// if feed_price is invalid, can not judge the cdp is safe or unsafe!
			false
		}
	}

	pub fn get_liquidation_ratio(currency_id: CurrencyIdOf<T>) -> Ratio {
		Self::liquidation_ratio(currency_id).unwrap_or_else(T::DefaultLiquidationRatio::get)
	}

	pub fn get_debit_exchange_rate(currency_id: CurrencyIdOf<T>) -> ExchangeRate {
		Self::debit_exchange_rate(currency_id).unwrap_or_else(T::DefaultDebitExchangeRate::get)
	}

	pub fn get_liquidation_penalty(currency_id: CurrencyIdOf<T>) -> Rate {
		Self::liquidation_penalty(currency_id).unwrap_or_else(T::DefaultLiquidationPenalty::get)
	}

	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}

	pub fn calculate_collateral_ratio(
		currency_id: CurrencyIdOf<T>,
		collateral_balance: BalanceOf<T>,
		debit_balance: T::DebitBalance,
		price: Price,
	) -> Ratio {
		let locked_collateral_value = price.saturating_mul_int(&collateral_balance);
		let debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));

		Ratio::from_rational(locked_collateral_value, debit_value)
	}

	pub fn exceed_debit_value_cap(currency_id: CurrencyIdOf<T>, debit_balance: T::DebitBalance) -> bool {
		let hard_cap = Self::maximum_total_debit_value(currency_id);
		let issue = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		issue > hard_cap
	}

	pub fn update_position(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: T::DebitAmount,
	) -> DispatchResult {
		ensure!(
			T::CollateralCurrencyIds::get().contains(&currency_id),
			Error::<T>::NotValidCurrencyId,
		);
		<loans::Module<T>>::update_position(who, currency_id, collateral_adjustment, debit_adjustment)
			.map_err(|_| Error::<T>::UpdatePositionFailed)?;

		Ok(())
	}

	// settle cdp has debit when emergency shutdown
	pub fn settle_cdp_has_debit(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		let debit_balance = <loans::Module<T>>::debits(currency_id, &who).0;
		ensure!(!debit_balance.is_zero(), Error::<T>::AlreadyNoDebit);

		// confiscate collateral in cdp to cdp treasury
		// and decrease cdp's debit to zero
		let collateral_balance = <loans::Module<T>>::collaterals(&who, currency_id);
		let settle_price: Price = T::PriceSource::get_price(currency_id, T::GetStableCurrencyId::get())
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let debt_in_stable_currency = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let confiscate_collateral_amount = rstd::cmp::min(
			settle_price.saturating_mul_int(&debt_in_stable_currency),
			collateral_balance,
		);
		let grab_collateral_amount = TryInto::<AmountOf<T>>::try_into(confiscate_collateral_amount)
			.map_err(|_| Error::<T>::AmountConvertFailed)?;
		let grab_debit_amount =
			TryInto::<T::DebitAmount>::try_into(debit_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;
		<loans::Module<T>>::update_collaterals_and_debits(
			who.clone(),
			currency_id,
			-grab_collateral_amount,
			-grab_debit_amount,
		)
		.map_err(|_| Error::<T>::GrabCollateralAndDebitFailed)?;
		<T as Trait>::Treasury::deposit_system_collateral(currency_id, confiscate_collateral_amount);
		<T as Trait>::Treasury::on_system_debit(debt_in_stable_currency);

		Self::deposit_event(RawEvent::SettleCdpInDebit(currency_id, who));
		Ok(())
	}

	// liquidate unsafe cdp
	pub fn liquidate_unsafe_cdp(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		let debit_balance = <loans::Module<T>>::debits(currency_id, &who).0;
		let collateral_balance = <loans::Module<T>>::collaterals(&who, currency_id);
		let stable_currency_id = T::GetStableCurrencyId::get();
		let feed_price =
			T::PriceSource::get_price(stable_currency_id, currency_id).ok_or(Error::<T>::InvalidFeedPrice)?;

		// first: ensure the cdp is unsafe
		ensure!(!debit_balance.is_zero(), Error::<T>::NoDebitInCdp);
		let collateral_ratio =
			Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);
		let liquidation_ratio = Self::get_liquidation_ratio(currency_id);
		ensure!(
			collateral_ratio < liquidation_ratio,
			Error::<T>::CollateralRatioStillSafe
		);

		// second: grab collaterals and debits from unsafe cdp
		let grab_amount =
			TryInto::<AmountOf<T>>::try_into(collateral_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;
		let grab_debit_amount =
			TryInto::<T::DebitAmount>::try_into(debit_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;
		<loans::Module<T>>::update_collaterals_and_debits(who.clone(), currency_id, -grab_amount, -grab_debit_amount)
			.map_err(|_| Error::<T>::GrabCollateralAndDebitFailed)?;

		// third: calculate bad_debt and target
		let bad_debt = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let target = bad_debt.saturating_add(Self::get_liquidation_penalty(currency_id).saturating_mul_int(&bad_debt));

		// add system debit to cdp treasury
		<T as Trait>::Treasury::on_system_debit(bad_debt);

		// if collateral_balance can swap enough native token in DEX and exchange slippage is blow the limit,
		// directly exchange with DEX, otherwise create collateral auctions.
		let supply_amount = T::Dex::get_supply_amount(currency_id, stable_currency_id, target);
		let slippage = T::Dex::get_exchange_slippage(currency_id, stable_currency_id, supply_amount);
		let slippage_limit = T::MaxSlippageSwapWithDex::get();

		// forth: handle bad debt and collateral
		if !supply_amount.is_zero() 				// supply_amount must not be zero
		&& collateral_balance >= supply_amount		// can afford supply_amount
		&& slippage_limit > Ratio::from_natural(0)	// slippage_limit must be greater than zero
		&& slippage.map_or(false, |s| s <= slippage_limit)
		{
			// directly exchange with DEX
			// deposit supply_amount collateral to cdp treasury
			<T as Trait>::Treasury::deposit_system_collateral(currency_id, supply_amount);

			// exchange with Dex by cdp treasury
			<T as Trait>::Treasury::swap_collateral_to_stable(currency_id, supply_amount, target);

			// refund remain collateral to who
			let refund_collateral_amount = collateral_balance - supply_amount;
			if !refund_collateral_amount.is_zero() {
				<T as Trait>::Currency::deposit(currency_id, &who, refund_collateral_amount).expect("never failed");
			}
		} else {
			// deposit collateral_balance collateral to cdp treasury
			<T as Trait>::Treasury::deposit_system_collateral(currency_id, collateral_balance);

			// create collateral auctions by cdp treasury
			<T as Trait>::Treasury::create_collateral_auctions(currency_id, collateral_balance, target, who.clone());
		}

		Self::deposit_event(RawEvent::LiquidateUnsafeCdp(
			currency_id,
			who,
			collateral_balance,
			bad_debt,
		));

		Ok(())
	}
}

impl<T: Trait> RiskManager<T::AccountId, CurrencyIdOf<T>, AmountOf<T>, T::DebitAmount> for Module<T> {
	fn check_position_adjustment(
		account_id: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_amount: AmountOf<T>,
		debit_amount: T::DebitAmount,
	) -> DispatchResult {
		let mut debit_balance = <loans::Module<T>>::debits(currency_id, account_id).0;
		let mut collateral_balance = <loans::Module<T>>::collaterals(account_id, currency_id);

		// calculate new debit balance and collateral balance after position adjustment
		let collateral_balance_adjustment =
			TryInto::<BalanceOf<T>>::try_into(collateral_amount.abs()).map_err(|_| Error::<T>::AmountConvertFailed)?;
		if collateral_amount.is_positive() {
			collateral_balance = collateral_balance
				.checked_add(&collateral_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		} else {
			collateral_balance = collateral_balance
				.checked_sub(&collateral_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		}

		let debit_balance_adjustment = TryInto::<T::DebitBalance>::try_into(debit_amount.abs())
			.map_err(|_| Error::<T>::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			debit_balance = debit_balance
				.checked_add(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		} else {
			debit_balance = debit_balance
				.checked_sub(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		}

		let debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));

		if !debit_value.is_zero() {
			// check the required collateral ratio
			let feed_price = <T as Trait>::PriceSource::get_price(T::GetStableCurrencyId::get(), currency_id)
				.ok_or(Error::<T>::InvalidFeedPrice)?;
			let collateral_ratio =
				Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);
			if let Some(required_collateral_ratio) = Self::required_collateral_ratio(currency_id) {
				ensure!(
					collateral_ratio >= required_collateral_ratio,
					Error::<T>::BelowRequiredCollateralRatio
				);
			}

			// check the liquidation ratio
			let liquidation_ratio = if let Some(ratio) = Self::liquidation_ratio(currency_id) {
				ratio
			} else {
				T::DefaultLiquidationRatio::get()
			};
			ensure!(collateral_ratio >= liquidation_ratio, Error::<T>::BelowLiquidationRatio);

			// check the minimum_debit_value
			ensure!(
				debit_value >= T::MinimumDebitValue::get(),
				Error::<T>::RemainDebitValueTooSmall,
			);
		}

		Ok(())
	}

	fn check_debit_cap(currency_id: CurrencyIdOf<T>, debit_amount: T::DebitAmount) -> DispatchResult {
		let mut total_debit_balance = <loans::Module<T>>::total_debits(currency_id);
		let debit_balance_adjustment = TryInto::<T::DebitBalance>::try_into(debit_amount.abs())
			.map_err(|_| Error::<T>::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			total_debit_balance = total_debit_balance
				.checked_add(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		} else {
			total_debit_balance = total_debit_balance
				.checked_sub(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		}
		ensure!(
			!Self::exceed_debit_value_cap(currency_id, total_debit_balance),
			Error::<T>::ExceedDebitValueHardCap
		);

		Ok(())
	}
}

impl<T: Trait> EmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		Self::emergency_shutdown();
	}
}

#[allow(deprecated)]
impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(call: &Self::Call) -> TransactionValidity {
		match call {
			Call::liquidate(currency_id, who) => {
				if !Self::is_unsafe_cdp(*currency_id, &who) || Self::is_shutdown() {
					return InvalidTransaction::Stale.into();
				}

				Ok(ValidTransaction {
					priority: TransactionPriority::max_value(),
					requires: vec![],
					provides: vec![(<system::Module<T>>::block_number(), currency_id, who).encode()],
					longevity: 64_u64,
					propagate: true,
				})
			}
			Call::settle(currency_id, who) => {
				let debit_balance = <loans::Module<T>>::debits(currency_id, who).0;
				if debit_balance.is_zero() || !Self::is_shutdown() {
					return InvalidTransaction::Stale.into();
				}

				Ok(ValidTransaction {
					priority: TransactionPriority::max_value(),
					requires: vec![],
					provides: vec![(<system::Module<T>>::block_number(), currency_id, who).encode()],
					longevity: 64_u64,
					propagate: true,
				})
			}
			_ => InvalidTransaction::Call.into(),
		}
	}
}
