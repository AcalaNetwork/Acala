//! # Loans Module
//!
//! ## Overview
//!
//! Loans module manages CDP's collateral assets and the debits backed by these
//! assets.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, traits::Get};
use frame_system::{self as system};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::with_transaction_result;
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, Convert, Zero},
	DispatchResult, ModuleId,
};
use sp_std::{convert::TryInto, result};
use support::{CDPTreasury, RiskManager};

mod mock;
mod tests;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// Convert debit amount under specific collateral type to debit
	/// value(stable coin)
	type Convert: Convert<(CurrencyId, Balance), Balance>;

	/// Currency type for deposit/withdraw collateral assets to/from loans
	/// module
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance, Amount = Amount>;

	/// Risk manager is used to limit the debit size of CDP
	type RiskManager: RiskManager<Self::AccountId, CurrencyId, Balance, Balance>;

	/// CDP treasury for issuing/burning stable coin adjust debit value
	/// adjustment
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// The loan's module id, keep all collaterals of CDPs.
	type ModuleId: Get<ModuleId>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Loans {
		/// The debit amount records of CDPs, map from
		/// CollateralType -> Owner -> DebitAmount
		pub Debits get(fn debits): double_map hasher(twox_64_concat) CurrencyId, hasher(twox_64_concat) T::AccountId => Balance;

		/// The collateral asset amount of CDPs, map from
		/// Owner -> CollateralType -> CollateralAmount
		pub Collaterals get(fn collaterals): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) CurrencyId => Balance;

		/// The total debit amount, map from
		/// CollateralType -> TotalDebitAmount
		pub TotalDebits get(fn total_debits): map hasher(twox_64_concat) CurrencyId => Balance;

		/// The total collateral asset amount, map from
		/// CollateralType -> TotalCollateralAmount
		pub TotalCollaterals get(fn total_collaterals): map hasher(twox_64_concat) CurrencyId => Balance;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		Amount = Amount,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Position updated. [owner, collateral_type, collateral_adjustment, debit_adjustment]
		PositionUpdated(AccountId, CurrencyId, Amount, Amount),
		/// Confiscate CDP's collateral assets and eliminate its debit. [owner, collateral_type, confiscated_collateral_amount, deduct_debit_amount]
		ConfiscateCollateralAndDebit(AccountId, CurrencyId, Balance, Balance),
		/// Transfer loan. [from, to, currency_id]
		TransferLoan(AccountId, AccountId, CurrencyId),
	}
);

decl_error! {
	/// Error for loans module.
	pub enum Error for Module<T: Trait> {
		DebitOverflow,
		DebitTooLow,
		CollateralOverflow,
		CollateralTooLow,
		AmountConvertFailed,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		/// The loan's module id, keep all collaterals of CDPs.
		const ModuleId: ModuleId = T::ModuleId::get();
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	// confiscate collateral and debit to cdp treasury
	pub fn confiscate_collateral_and_debit(
		who: &T::AccountId,
		currency_id: CurrencyId,
		collateral_confiscate: Balance,
		debit_decrease: Balance,
	) -> DispatchResult {
		with_transaction_result(|| -> DispatchResult {
			// use `with_transaction_result` to ensure operation is atomic
			// convert balance type to amount type
			let collateral_adjustment = Self::amount_try_from_balance(collateral_confiscate)?;
			let debit_adjustment = Self::amount_try_from_balance(debit_decrease)?;

			// transfer collateral to cdp treasury
			T::CDPTreasury::deposit_collateral(&Self::account_id(), currency_id, collateral_confiscate)?;

			// deposit debit to cdp treasury
			let bad_debt_value = T::RiskManager::get_bad_debt_value(currency_id, debit_decrease);
			T::CDPTreasury::on_system_debit(bad_debt_value)?;

			// update loan
			Self::update_loan(
				&who,
				currency_id,
				collateral_adjustment.saturating_neg(),
				debit_adjustment.saturating_neg(),
			)?;

			Self::deposit_event(RawEvent::ConfiscateCollateralAndDebit(
				who.clone(),
				currency_id,
				collateral_confiscate,
				debit_decrease,
			));
			Ok(())
		})
	}

	// mutate collaterals and debits and then mutate stable coin
	pub fn adjust_position(
		who: &T::AccountId,
		currency_id: CurrencyId,
		collateral_adjustment: Amount,
		debit_adjustment: Amount,
	) -> DispatchResult {
		with_transaction_result(|| -> DispatchResult {
			// use `with_transaction_result` to ensure operation is atomic
			// mutate collateral and debit
			Self::update_loan(who, currency_id, collateral_adjustment, debit_adjustment)?;

			let collateral_balance_adjustment = Self::balance_try_from_amount_abs(collateral_adjustment)?;
			let debit_balance_adjustment = Self::balance_try_from_amount_abs(debit_adjustment)?;
			let module_account = Self::account_id();

			if collateral_adjustment.is_positive() {
				T::Currency::transfer(currency_id, who, &module_account, collateral_balance_adjustment)?;
			} else if collateral_adjustment.is_negative() {
				T::Currency::transfer(currency_id, &module_account, who, collateral_balance_adjustment)?;
			}

			if debit_adjustment.is_positive() {
				// check debit cap when increase debit
				T::RiskManager::check_debit_cap(currency_id, Self::total_debits(currency_id))?;

				// issue debit with collateral backed by cdp treasury
				T::CDPTreasury::issue_debit(who, T::Convert::convert((currency_id, debit_balance_adjustment)), true)?;
			} else if debit_adjustment.is_negative() {
				// repay debit
				// burn debit by cdp treasury
				T::CDPTreasury::burn_debit(who, T::Convert::convert((currency_id, debit_balance_adjustment)))?;
			}

			// ensure pass risk check
			T::RiskManager::check_position_valid(
				currency_id,
				Self::collaterals(who, currency_id),
				Self::debits(currency_id, who),
			)?;

			Self::deposit_event(RawEvent::PositionUpdated(
				who.clone(),
				currency_id,
				collateral_adjustment,
				debit_adjustment,
			));
			Ok(())
		})
	}

	// transfer whole loan of `from` to `to`
	pub fn transfer_loan(from: &T::AccountId, to: &T::AccountId, currency_id: CurrencyId) -> DispatchResult {
		// get `from` position data
		let collateral_balance = Self::collaterals(from, currency_id);
		let debit_balance = Self::debits(currency_id, from);

		let new_to_collateral_balance = Self::collaterals(to, currency_id)
			.checked_add(collateral_balance)
			.expect("existing collateral balance cannot overflow; qed");
		let new_to_debit_balance = Self::debits(currency_id, to)
			.checked_add(debit_balance)
			.expect("existing debit balance cannot overflow; qed");

		// check new position
		T::RiskManager::check_position_valid(currency_id, new_to_collateral_balance, new_to_debit_balance)?;

		// balance -> amount
		let collateral_adjustment = Self::amount_try_from_balance(collateral_balance)?;
		let debit_adjustment = Self::amount_try_from_balance(debit_balance)?;

		Self::update_loan(
			from,
			currency_id,
			collateral_adjustment.saturating_neg(),
			debit_adjustment.saturating_neg(),
		)?;
		Self::update_loan(to, currency_id, collateral_adjustment, debit_adjustment)?;

		Self::deposit_event(RawEvent::TransferLoan(from.clone(), to.clone(), currency_id));
		Ok(())
	}

	fn update_loan(
		who: &T::AccountId,
		currency_id: CurrencyId,
		collateral_adjustment: Amount,
		debit_adjustment: Amount,
	) -> DispatchResult {
		let collateral_balance = Self::balance_try_from_amount_abs(collateral_adjustment)?;
		let debit_balance = Self::balance_try_from_amount_abs(debit_adjustment)?;

		// update collateral record
		if collateral_adjustment.is_positive() {
			TotalCollaterals::try_mutate(currency_id, |balance| -> DispatchResult {
				*balance = balance
					.checked_add(collateral_balance)
					.ok_or(Error::<T>::CollateralOverflow)?;
				Ok(())
			})?;
			<Collaterals<T>>::try_mutate(who, currency_id, |balance| -> DispatchResult {
				// increase account ref for who when has no amount before
				if balance.is_zero() {
					system::Module::<T>::inc_ref(who);
				}
				*balance = balance
					.checked_add(collateral_balance)
					.expect("collateral cannot overflow if total collateral does not; qed");
				Ok(())
			})?;
		} else if collateral_adjustment.is_negative() {
			<Collaterals<T>>::try_mutate(who, currency_id, |balance| -> DispatchResult {
				*balance = balance
					.checked_sub(collateral_balance)
					.ok_or(Error::<T>::CollateralTooLow)?;
				// decrease account ref for who when has no amount
				if balance.is_zero() {
					system::Module::<T>::dec_ref(who);
				}
				Ok(())
			})?;
			TotalCollaterals::try_mutate(currency_id, |balance| -> DispatchResult {
				*balance = balance
					.checked_sub(collateral_balance)
					.expect("total collateral cannot underflow if collateral does not; qed");
				Ok(())
			})?;
		}

		// update debit record
		if debit_adjustment.is_positive() {
			TotalDebits::try_mutate(currency_id, |balance| -> DispatchResult {
				*balance = balance.checked_add(debit_balance).ok_or(Error::<T>::DebitOverflow)?;
				Ok(())
			})?;
			<Debits<T>>::try_mutate(currency_id, who, |balance| -> DispatchResult {
				*balance = balance
					.checked_add(debit_balance)
					.expect("debit cannot overflow if total debit does not; qed");
				Ok(())
			})?;
		} else if debit_adjustment.is_negative() {
			<Debits<T>>::try_mutate(currency_id, who, |balance| -> DispatchResult {
				*balance = balance.checked_sub(debit_balance).ok_or(Error::<T>::DebitTooLow)?;
				Ok(())
			})?;
			TotalDebits::try_mutate(currency_id, |balance| -> DispatchResult {
				*balance = balance
					.checked_sub(debit_balance)
					.expect("total debit cannot underflow if debit does not; qed");
				Ok(())
			})?;
		}

		Ok(())
	}
}

impl<T: Trait> Module<T> {
	/// Convert `Balance` to `Amount`.
	fn amount_try_from_balance(b: Balance) -> result::Result<Amount, Error<T>> {
		TryInto::<Amount>::try_into(b).map_err(|_| Error::<T>::AmountConvertFailed)
	}

	/// Convert the absolute value of `Amount` to `Balance`.
	fn balance_try_from_amount_abs(a: Amount) -> result::Result<Balance, Error<T>> {
		TryInto::<Balance>::try_into(a.saturating_abs()).map_err(|_| Error::<T>::AmountConvertFailed)
	}
}
