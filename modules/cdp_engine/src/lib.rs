#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, IsSubType, IterableStorageDoubleMap,
};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use rstd::{marker, prelude::*};
use sp_runtime::{
	traits::{BlakeTwo256, Convert, EnsureOrigin, Hash, Saturating, UniqueSaturatedInto, Zero},
	transaction_validity::{InvalidTransaction, TransactionPriority, TransactionValidity, ValidTransaction},
	DispatchResult, RandomNumberGenerator,
};
use support::{
	CDPTreasury, CDPTreasuryExtended, DEXManager, EmergencyShutdown, ExchangeRate, Price, PriceProvider, Rate, Ratio,
	RiskManager,
};
use system::{ensure_none, ensure_root, offchain::SubmitUnsignedTransaction};
use utilities::{LockItem, OffchainErr, OffchainLock};

mod debit_exchange_rate_convertor;
pub use debit_exchange_rate_convertor::DebitExchangeRateConvertor;

mod mock;
mod tests;

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
	type CDPTreasury: CDPTreasuryExtended<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
	type UpdateOrigin: EnsureOrigin<Self::Origin>;
	type MaxSlippageSwapWithDEX: Get<Ratio>;
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyIdOf<Self>, Balance = BalanceOf<Self>>;
	type DEX: DEXManager<Self::AccountId, CurrencyIdOf<Self>, BalanceOf<Self>>;

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
		LiquidateUnsafeCDP(CurrencyId, AccountId, Balance, Balance),
		SettleCDPInDebit(CurrencyId, AccountId),
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
		BelowRequiredCollateralRatio,
		BelowLiquidationRatio,
		MustBeUnsafe,
		InvalidCurrencyId,
		RemainDebitValueTooSmall,
		InvalidFeedPrice,
		AlreadyNoDebit,
		AlreadyShutdown,
		MustAfterShutdown,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as CDPEngine {
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

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const CollateralCurrencyIds: Vec<CurrencyIdOf<T>> = T::CollateralCurrencyIds::get();
		const GlobalStabilityFee: Rate = T::GlobalStabilityFee::get();
		const DefaultLiquidationRatio: Ratio = T::DefaultLiquidationRatio::get();
		const DefaultDebitExchangeRate: ExchangeRate = T::DefaultDebitExchangeRate::get();
		const MinimumDebitValue: BalanceOf<T> = T::MinimumDebitValue::get();
		const GetStableCurrencyId: CurrencyIdOf<T> = T::GetStableCurrencyId::get();
		const MaxSlippageSwapWithDEX: Ratio = T::MaxSlippageSwapWithDEX::get();
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
						let total_debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, total_debits));
						let issued_stable_coin_balance = debit_exchange_rate_increment.saturating_mul_int(&total_debit_value);

						// issue stablecoin to surplus pool
						if <T as Trait>::CDPTreasury::on_system_surplus(issued_stable_coin_balance).is_ok() {
							// update exchange rate when issue success
							let new_debit_exchange_rate = debit_exchange_rate.saturating_add(debit_exchange_rate_increment);
							<DebitExchangeRate<T>>::insert(currency_id, new_debit_exchange_rate);
						}
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

	fn _offchain_worker(block_number: T::BlockNumber) -> Result<(), OffchainErr> {
		let collateral_currency_ids = T::CollateralCurrencyIds::get();
		if collateral_currency_ids.len().is_zero() {
			return Ok(());
		}

		// check if we are a potential validator
		if !runtime_io::offchain::is_validator() {
			return Err(OffchainErr::NotValidator);
		}

		let collateral_currency_ids = T::CollateralCurrencyIds::get();
		let offchain_lock = OffchainLock::new(DB_PREFIX.to_vec());

		// Acquire offchain worker lock.
		// If succeeded, update the lock, otherwise return error
		let LockItem {
			expire_timestamp: _,
			extra_data: position,
		} = offchain_lock.acquire_offchain_lock(|val: Option<u32>| {
			if let Some(previous_position) = val {
				if previous_position < collateral_currency_ids.len().saturating_sub(1) as u32 {
					previous_position + 1
				} else {
					0
				}
			} else {
				let random_seed = runtime_io::offchain::random_seed();
				let mut rng = RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(&random_seed[..]));

				rng.pick_u32(collateral_currency_ids.len().saturating_sub(1) as u32)
			}
		})?;

		let currency_id = collateral_currency_ids[(position as usize)];

		if !Self::is_shutdown() {
			for (account_id, _) in <loans::Debits<T>>::iter(currency_id) {
				if Self::is_cdp_unsafe(currency_id, &account_id) {
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

				// check the expire timestamp of lock that is needed to extend
				offchain_lock.extend_offchain_lock_if_needed::<u32>();
			}
		} else {
			for (account_id, debit) in <loans::Debits<T>>::iter(currency_id) {
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

				// check the expire timestamp of lock that is needed to extend
				offchain_lock.extend_offchain_lock_if_needed::<u32>();
			}
		}

		// finally, reset the expire timestamp to now in order to release lock in advance.
		offchain_lock.release_offchain_lock(|current_position: u32| current_position == position);
		debug::debug!(
			target: "cdp-engine offchain worker",
			"offchain worker start at block: {:?} already done!",
			block_number,
		);

		Ok(())
	}

	pub fn is_cdp_unsafe(currency_id: CurrencyIdOf<T>, who: &T::AccountId) -> bool {
		let debit_balance = <loans::Module<T>>::debits(currency_id, who);
		let collateral_balance = <loans::Module<T>>::collaterals(who, currency_id);
		let stable_currency_id = T::GetStableCurrencyId::get();

		if debit_balance.is_zero() {
			false
		} else if let Some(feed_price) = T::PriceSource::get_price(stable_currency_id, currency_id) {
			let collateral_ratio =
				Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);
			collateral_ratio < Self::get_liquidation_ratio(currency_id)
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

	pub fn adjust_position(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: T::DebitAmount,
	) -> DispatchResult {
		ensure!(
			T::CollateralCurrencyIds::get().contains(&currency_id),
			Error::<T>::InvalidCurrencyId,
		);
		<loans::Module<T>>::adjust_position(who, currency_id, collateral_adjustment, debit_adjustment)?;
		Ok(())
	}

	// settle cdp has debit when emergency shutdown
	pub fn settle_cdp_has_debit(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		let debit_balance = <loans::Module<T>>::debits(currency_id, &who);
		ensure!(!debit_balance.is_zero(), Error::<T>::AlreadyNoDebit);

		// confiscate collateral in cdp to cdp treasury
		// and decrease cdp's debit to zero
		let collateral_balance = <loans::Module<T>>::collaterals(&who, currency_id);
		let settle_price: Price = T::PriceSource::get_price(currency_id, T::GetStableCurrencyId::get())
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let bad_debt_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let confiscate_collateral_amount =
			rstd::cmp::min(settle_price.saturating_mul_int(&bad_debt_value), collateral_balance);

		// confiscate collateral and all debit
		<loans::Module<T>>::confiscate_collateral_and_debit(
			&who,
			currency_id,
			confiscate_collateral_amount,
			debit_balance,
		)?;
		<T as Trait>::CDPTreasury::on_system_debit(bad_debt_value)?;

		Self::deposit_event(RawEvent::SettleCDPInDebit(currency_id, who));
		Ok(())
	}

	// liquidate unsafe cdp
	pub fn liquidate_unsafe_cdp(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		let debit_balance = <loans::Module<T>>::debits(currency_id, &who);
		let collateral_balance = <loans::Module<T>>::collaterals(&who, currency_id);
		let stable_currency_id = T::GetStableCurrencyId::get();

		// ensure the cdp is unsafe
		ensure!(Self::is_cdp_unsafe(currency_id, &who), Error::<T>::MustBeUnsafe,);

		// confiscate all collateral and all debit from unsafe cdp
		<loans::Module<T>>::confiscate_collateral_and_debit(&who, currency_id, collateral_balance, debit_balance)?;

		// calculate bad_debt_value and target_value
		let bad_debt_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let target_value = bad_debt_value
			.saturating_add(Self::get_liquidation_penalty(currency_id).saturating_mul_int(&bad_debt_value));

		// add system debit to cdp treasury
		<T as Trait>::CDPTreasury::on_system_debit(bad_debt_value)?;

		// if collateral_balance can swap enough native token in DEX and exchange slippage is blow the limit,
		// directly exchange with DEX, otherwise create collateral auctions.
		let supply_amount = T::DEX::get_supply_amount(currency_id, stable_currency_id, target_value);
		let slippage = T::DEX::get_exchange_slippage(currency_id, stable_currency_id, supply_amount);
		let slippage_limit = T::MaxSlippageSwapWithDEX::get();

		// handle bad debt and collateral
		if !supply_amount.is_zero() 				// supply_amount must not be zero
		&& collateral_balance >= supply_amount		// ensure have sufficient collateral
		&& slippage_limit > Ratio::from_natural(0)	// slippage_limit must be greater than zero
		&& slippage.map_or(false, |s| s <= slippage_limit)
		&& T::DEX::get_target_amount(currency_id, stable_currency_id, supply_amount) >= target_value
		// ensure supply can afford target
		{
			// exchange with DEX by cdp treasury
			if <T as Trait>::CDPTreasury::swap_collateral_to_stable(currency_id, supply_amount, target_value).is_ok() {
				// refund remain collateral to who
				let refund_collateral_amount = collateral_balance - supply_amount;
				if !refund_collateral_amount.is_zero() {
					<T as Trait>::CDPTreasury::transfer_system_collateral(currency_id, &who, refund_collateral_amount)
						.expect("never failed");
				}
			}
		} else {
			// create collateral auctions by cdp treasury
			<T as Trait>::CDPTreasury::create_collateral_auctions(
				currency_id,
				collateral_balance,
				target_value,
				who.clone(),
			);
		}

		Self::deposit_event(RawEvent::LiquidateUnsafeCDP(
			currency_id,
			who,
			collateral_balance,
			bad_debt_value,
		));

		Ok(())
	}
}

impl<T: Trait> RiskManager<T::AccountId, CurrencyIdOf<T>, BalanceOf<T>, T::DebitBalance> for Module<T> {
	fn check_position_valid(
		currency_id: CurrencyIdOf<T>,
		collateral_balance: BalanceOf<T>,
		debit_balance: T::DebitBalance,
	) -> DispatchResult {
		let debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));

		if !debit_value.is_zero() {
			let feed_price = <T as Trait>::PriceSource::get_price(T::GetStableCurrencyId::get(), currency_id)
				.ok_or(Error::<T>::InvalidFeedPrice)?;
			let collateral_ratio =
				Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);

			// check the required collateral ratio
			if let Some(required_collateral_ratio) = Self::required_collateral_ratio(currency_id) {
				ensure!(
					collateral_ratio >= required_collateral_ratio,
					Error::<T>::BelowRequiredCollateralRatio
				);
			}

			// check the liquidation ratio
			ensure!(
				collateral_ratio >= Self::get_liquidation_ratio(currency_id),
				Error::<T>::BelowLiquidationRatio
			);

			// check the minimum_debit_value
			ensure!(
				debit_value >= T::MinimumDebitValue::get(),
				Error::<T>::RemainDebitValueTooSmall,
			);
		}

		Ok(())
	}

	fn check_debit_cap(currency_id: CurrencyIdOf<T>, total_debit_balance: T::DebitBalance) -> DispatchResult {
		let hard_cap = Self::maximum_total_debit_value(currency_id);
		let total_debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, total_debit_balance));

		ensure!(total_debit_value <= hard_cap, Error::<T>::ExceedDebitValueHardCap,);

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
				if !Self::is_cdp_unsafe(*currency_id, &who) || Self::is_shutdown() {
					return InvalidTransaction::Stale.into();
				}

				Ok(ValidTransaction {
					priority: TransactionPriority::max_value(),
					requires: vec![],
					provides: vec![(
						"CDPEngineOffchain",
						<system::Module<T>>::block_number(),
						currency_id,
						who,
					)
						.encode()],
					longevity: 64_u64,
					propagate: true,
				})
			}
			Call::settle(currency_id, who) => {
				let debit_balance = <loans::Module<T>>::debits(currency_id, who);
				if debit_balance.is_zero() || !Self::is_shutdown() {
					return InvalidTransaction::Stale.into();
				}

				Ok(ValidTransaction {
					priority: TransactionPriority::max_value(),
					requires: vec![],
					provides: vec![("CDPEngineOffchain", currency_id, who).encode()],
					longevity: 64_u64,
					propagate: true,
				})
			}
			_ => InvalidTransaction::Call.into(),
		}
	}
}
