//! # Loans Module
//!
//! ## Overview
//!
//! Loans module manages CDP's collateral assets and the debits backed by these
//! assets.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	traits::{Get, Happened},
};
use frame_system::{self as system};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::with_transaction_result;
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, Convert, Zero},
	DispatchResult, ModuleId, RuntimeDebug,
};
use sp_std::{convert::TryInto, result};
use support::{CDPTreasury, RiskManager};

mod mock;
mod tests;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// Convert debit amount under specific collateral type to debit
	/// value(stable currency)
	type Convert: Convert<(CurrencyId, Balance), Balance>;

	/// Currency type for deposit/withdraw collateral assets to/from loans
	/// module
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance, Amount = Amount>;

	/// Risk manager is used to limit the debit size of CDP
	type RiskManager: RiskManager<Self::AccountId, CurrencyId, Balance, Balance>;

	/// CDP treasury for issuing/burning stable currency adjust debit value
	/// adjustment
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// The loan's module id, keep all collaterals of CDPs.
	type ModuleId: Get<ModuleId>;

	/// Event handler which calls when update loan.
	type OnUpdateLoan: Happened<(Self::AccountId, CurrencyId, Amount, Balance)>;
}

/// A collateralized debit position.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, Default)]
pub struct Position {
	/// The amount of collateral.
	pub collateral: Balance,
	/// The amount of debit.
	pub debit: Balance,
}

decl_storage! {
	trait Store for Module<T: Trait> as Loans {
		/// The collateralized debit positions, map from
		/// Owner -> CollateralType -> Position
		pub Positions get(fn positions): double_map hasher(twox_64_concat) CurrencyId, hasher(twox_64_concat) T::AccountId => Position;

		/// The total collateralized debit positions, map from
		/// CollateralType -> Position
		pub TotalPositions get(fn total_positions): map hasher(twox_64_concat) CurrencyId => Position;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		Amount = Amount,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Position updated. \[owner, collateral_type, collateral_adjustment, debit_adjustment\]
		PositionUpdated(AccountId, CurrencyId, Amount, Amount),
		/// Confiscate CDP's collateral assets and eliminate its debit. \[owner, collateral_type, confiscated_collateral_amount, deduct_debit_amount\]
		ConfiscateCollateralAndDebit(AccountId, CurrencyId, Balance, Balance),
		/// Transfer loan. \[from, to, currency_id\]
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

	/// confiscate collateral and debit to cdp treasury
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

	/// adjust the position
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
				T::RiskManager::check_debit_cap(currency_id, Self::total_positions(currency_id).debit)?;

				// issue debit with collateral backed by cdp treasury
				T::CDPTreasury::issue_debit(who, T::Convert::convert((currency_id, debit_balance_adjustment)), true)?;
			} else if debit_adjustment.is_negative() {
				// repay debit
				// burn debit by cdp treasury
				T::CDPTreasury::burn_debit(who, T::Convert::convert((currency_id, debit_balance_adjustment)))?;
			}

			// ensure pass risk check
			let Position { collateral, debit } = Self::positions(currency_id, who);
			T::RiskManager::check_position_valid(currency_id, collateral, debit)?;

			Self::deposit_event(RawEvent::PositionUpdated(
				who.clone(),
				currency_id,
				collateral_adjustment,
				debit_adjustment,
			));
			Ok(())
		})
	}

	/// transfer whole loan of `from` to `to`
	pub fn transfer_loan(from: &T::AccountId, to: &T::AccountId, currency_id: CurrencyId) -> DispatchResult {
		// get `from` position data
		let Position { collateral, debit } = Self::positions(currency_id, from);

		let Position {
			collateral: to_collateral,
			debit: to_debit,
		} = Self::positions(currency_id, to);
		let new_to_collateral_balance = to_collateral
			.checked_add(collateral)
			.expect("existing collateral balance cannot overflow; qed");
		let new_to_debit_balance = to_debit
			.checked_add(debit)
			.expect("existing debit balance cannot overflow; qed");

		// check new position
		T::RiskManager::check_position_valid(currency_id, new_to_collateral_balance, new_to_debit_balance)?;

		// balance -> amount
		let collateral_adjustment = Self::amount_try_from_balance(collateral)?;
		let debit_adjustment = Self::amount_try_from_balance(debit)?;

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

	/// mutate records of collaterals and debits
	fn update_loan(
		who: &T::AccountId,
		currency_id: CurrencyId,
		collateral_adjustment: Amount,
		debit_adjustment: Amount,
	) -> DispatchResult {
		let collateral_balance = Self::balance_try_from_amount_abs(collateral_adjustment)?;
		let debit_balance = Self::balance_try_from_amount_abs(debit_adjustment)?;

		<Positions<T>>::try_mutate_exists(currency_id, who, |may_be_position| -> DispatchResult {
			let mut p = may_be_position.take().unwrap_or_default();
			let new_collateral = if collateral_adjustment.is_positive() {
				p.collateral
					.checked_add(collateral_balance)
					.ok_or(Error::<T>::CollateralOverflow)
			} else {
				p.collateral
					.checked_sub(collateral_balance)
					.ok_or(Error::<T>::CollateralTooLow)
			}?;
			let new_debit = if debit_adjustment.is_positive() {
				p.debit.checked_add(debit_balance).ok_or(Error::<T>::DebitOverflow)
			} else {
				p.debit.checked_sub(debit_balance).ok_or(Error::<T>::DebitTooLow)
			}?;

			// increase account ref if new position
			if p.collateral.is_zero() && p.debit.is_zero() {
				system::Module::<T>::inc_ref(who);
			}

			p.collateral = new_collateral;

			T::OnUpdateLoan::happened(&(who.clone(), currency_id, debit_adjustment, p.debit));
			p.debit = new_debit;

			if p.collateral.is_zero() && p.debit.is_zero() {
				// decrease account ref if zero position
				system::Module::<T>::dec_ref(who);

				// remove position storage if zero position
				*may_be_position = None;
			} else {
				*may_be_position = Some(p);
			}

			Ok(())
		})?;

		TotalPositions::try_mutate(currency_id, |total_positions| -> DispatchResult {
			total_positions.collateral = if collateral_adjustment.is_positive() {
				total_positions
					.collateral
					.checked_add(collateral_balance)
					.ok_or(Error::<T>::CollateralOverflow)
			} else {
				total_positions
					.collateral
					.checked_sub(collateral_balance)
					.ok_or(Error::<T>::CollateralTooLow)
			}?;

			total_positions.debit = if debit_adjustment.is_positive() {
				total_positions
					.debit
					.checked_add(debit_balance)
					.ok_or(Error::<T>::DebitOverflow)
			} else {
				total_positions
					.debit
					.checked_sub(debit_balance)
					.ok_or(Error::<T>::DebitTooLow)
			}?;

			Ok(())
		})
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
