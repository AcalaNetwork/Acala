#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, Parameter};
use orml_traits::{
	arithmetic::{self, Signed},
	MultiCurrency, MultiCurrencyExtended,
};
use rstd::convert::{TryFrom, TryInto};
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32Bit, CheckedAdd, CheckedSub, Convert, MaybeSerializeDeserialize, Member, Zero,
	},
	DispatchResult, ModuleId,
};
use support::{CDPTreasury, RiskManager};

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/loan");

type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AmountOf<T> = <<T as Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Convert: Convert<(CurrencyIdOf<Self>, Self::DebitBalance), BalanceOf<Self>>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type RiskManager: RiskManager<Self::AccountId, CurrencyIdOf<Self>, BalanceOf<Self>, Self::DebitBalance>;
	type DebitBalance: Parameter + Member + AtLeast32Bit + Default + Copy + MaybeSerializeDeserialize;
	type DebitAmount: Signed
		+ TryInto<Self::DebitBalance>
		+ TryFrom<Self::DebitBalance>
		+ Parameter
		+ Member
		+ arithmetic::SimpleArithmetic
		+ Default
		+ Copy
		+ MaybeSerializeDeserialize;
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Loans {
		pub Debits get(fn debits): double_map hasher(twox_64_concat) CurrencyIdOf<T>, hasher(twox_64_concat) T::AccountId => T::DebitBalance;
		pub Collaterals get(fn collaterals): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) CurrencyIdOf<T> => BalanceOf<T>;
		pub TotalDebits get(fn total_debits): map hasher(twox_64_concat) CurrencyIdOf<T> => T::DebitBalance;
		pub TotalCollaterals get(fn total_collaterals): map hasher(twox_64_concat) CurrencyIdOf<T> => BalanceOf<T>;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyIdOf<T>,
		<T as Trait>::DebitAmount,
		Amount = AmountOf<T>,
		<T as Trait>::DebitBalance,
		Balance = BalanceOf<T>,
	{
		/// Update Position success (account, currency_id, collaterals, debits)
		UpdatePosition(AccountId, CurrencyId, Amount, DebitAmount),
		/// confiscate collateral and deduct debit
		ConfiscateCollateralAndDebit(AccountId, CurrencyId, Balance, DebitBalance),
		/// Transfer loan (from, to)
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
		fn deposit_event() = default;
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	// confiscate collateral and debit to cdp treasury
	pub fn confiscate_collateral_and_debit(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_confiscate: BalanceOf<T>,
		debit_decrease: T::DebitBalance,
	) -> DispatchResult {
		// balance -> amount
		let collateral_adjustment =
			TryInto::<AmountOf<T>>::try_into(collateral_confiscate).map_err(|_| Error::<T>::AmountConvertFailed)?;
		let debit_adjustment =
			TryInto::<T::DebitAmount>::try_into(debit_decrease).map_err(|_| Error::<T>::AmountConvertFailed)?;

		// check update overflow
		Self::check_update_loan_overflow(who, currency_id, -collateral_adjustment, -debit_adjustment)?;

		// transfer collateral to cdp treasury
		T::CDPTreasury::transfer_collateral_from(currency_id, &Self::account_id(), collateral_confiscate)?;

		// deposit debit to cdp treasury
		let bad_debt_value = T::RiskManager::get_bad_debt_value(currency_id, debit_decrease);
		T::CDPTreasury::on_system_debit(bad_debt_value)?;

		// update loan
		Self::update_loan(&who, currency_id, -collateral_adjustment, -debit_adjustment)
			.expect("never failed ensured by overflow check");

		Self::deposit_event(RawEvent::ConfiscateCollateralAndDebit(
			who.clone(),
			currency_id,
			collateral_confiscate,
			debit_decrease,
		));
		Ok(())
	}

	// mutate collaterals and debits and then mutate stable coin
	pub fn adjust_position(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: T::DebitAmount,
	) -> DispatchResult {
		Self::check_update_loan_overflow(who, currency_id, collateral_adjustment, debit_adjustment)?;

		let collateral_balance_adjustment = TryInto::<BalanceOf<T>>::try_into(collateral_adjustment.abs())
			.map_err(|_| Error::<T>::AmountConvertFailed)?;
		let debit_balance_adjustment = TryInto::<T::DebitBalance>::try_into(debit_adjustment.abs())
			.map_err(|_| Error::<T>::AmountConvertFailed)?;

		let module_account = Self::account_id();
		let mut new_collateral_balance = Self::collaterals(who, currency_id);
		let mut new_debit_balance = Self::debits(currency_id, who);

		if collateral_adjustment.is_positive() {
			T::Currency::ensure_can_withdraw(currency_id, who, collateral_balance_adjustment)?;
			new_collateral_balance += collateral_balance_adjustment;
		} else if collateral_adjustment.is_negative() {
			T::Currency::ensure_can_withdraw(currency_id, &module_account, collateral_balance_adjustment)?;
			new_collateral_balance -= collateral_balance_adjustment;
		}

		if debit_adjustment.is_positive() {
			let new_total_debit_balance = Self::total_debits(currency_id) + debit_balance_adjustment;
			// check debit cap when increase debit
			T::RiskManager::check_debit_cap(currency_id, new_total_debit_balance)?;
			new_debit_balance += debit_balance_adjustment;
		} else if debit_adjustment.is_negative() {
			new_debit_balance -= debit_balance_adjustment;
		}

		// ensure pass risk check
		T::RiskManager::check_position_valid(currency_id, new_collateral_balance, new_debit_balance)?;

		// update stable coin by Treasury
		if debit_adjustment.is_positive() {
			T::CDPTreasury::deposit_backed_debit_to(who, T::Convert::convert((currency_id, debit_balance_adjustment)))?;
		} else if debit_adjustment.is_negative() {
			T::CDPTreasury::withdraw_backed_debit_from(
				who,
				T::Convert::convert((currency_id, debit_balance_adjustment)),
			)?;
		}

		// update collateral asset
		if collateral_adjustment.is_positive() {
			T::Currency::transfer(currency_id, who, &module_account, collateral_balance_adjustment)
				.expect("never failed ensured by balance check");
		} else if collateral_adjustment.is_negative() {
			T::Currency::transfer(currency_id, &module_account, who, collateral_balance_adjustment)
				.expect("never failed ensured by balance check");
		}

		// mutate collateral and debit
		Self::update_loan(who, currency_id, collateral_adjustment, debit_adjustment)
			.expect("Will never fail ensured by overflow check");

		Self::deposit_event(RawEvent::UpdatePosition(
			who.clone(),
			currency_id,
			collateral_adjustment,
			debit_adjustment,
		));
		Ok(())
	}

	// transfer whole loan of `from` to `to`
	pub fn transfer_loan(from: &T::AccountId, to: &T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		// get `from` position data
		let collateral_balance = Self::collaterals(from, currency_id);
		let debit_balance = Self::debits(currency_id, from);

		let new_to_collateral_balance = Self::collaterals(to, currency_id) + collateral_balance;
		let new_to_debit_balance = Self::debits(currency_id, to) + debit_balance;

		// check new position
		T::RiskManager::check_position_valid(currency_id, new_to_collateral_balance, new_to_debit_balance)?;

		// balance -> amount
		let collateral_adjustment =
			TryInto::<AmountOf<T>>::try_into(collateral_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;
		let debit_adjustment =
			TryInto::<T::DebitAmount>::try_into(debit_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;

		Self::update_loan(from, currency_id, -collateral_adjustment, -debit_adjustment)?;
		Self::update_loan(to, currency_id, collateral_adjustment, debit_adjustment)?;

		Self::deposit_event(RawEvent::TransferLoan(from.clone(), to.clone(), currency_id));
		Ok(())
	}

	// check overflow for update_loan function
	pub fn check_update_loan_overflow(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: T::DebitAmount,
	) -> DispatchResult {
		let collateral_balance = TryInto::<BalanceOf<T>>::try_into(collateral_adjustment.abs())
			.map_err(|_| Error::<T>::AmountConvertFailed)?;
		let debit_balance = TryInto::<T::DebitBalance>::try_into(debit_adjustment.abs())
			.map_err(|_| Error::<T>::AmountConvertFailed)?;

		if collateral_adjustment.is_positive() {
			Self::total_collaterals(currency_id)
				.checked_add(&collateral_balance)
				.ok_or(Error::<T>::CollateralOverflow)?;
		} else if collateral_adjustment.is_negative() {
			Self::collaterals(who, currency_id)
				.checked_sub(&collateral_balance)
				.ok_or(Error::<T>::CollateralTooLow)?;
		}

		if debit_adjustment.is_positive() {
			Self::total_debits(currency_id)
				.checked_add(&debit_balance)
				.ok_or(Error::<T>::DebitOverflow)?;
		} else if debit_adjustment.is_negative() {
			Self::debits(currency_id, who)
				.checked_sub(&debit_balance)
				.ok_or(Error::<T>::DebitTooLow)?;
		}

		Ok(())
	}

	fn update_loan(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: T::DebitAmount,
	) -> DispatchResult {
		// check overflow first
		Self::check_update_loan_overflow(who, currency_id, collateral_adjustment, debit_adjustment)?;

		let collateral_balance = TryInto::<BalanceOf<T>>::try_into(collateral_adjustment.abs())
			.map_err(|_| Error::<T>::AmountConvertFailed)?;
		let debit_balance = TryInto::<T::DebitBalance>::try_into(debit_adjustment.abs())
			.map_err(|_| Error::<T>::AmountConvertFailed)?;

		// update collateral record
		if collateral_adjustment.is_positive() {
			<Collaterals<T>>::mutate(who, currency_id, |balance| {
				// increase account ref for who when has no amount before
				if balance.is_zero() {
					system::Module::<T>::inc_ref(who);
				}
				*balance += collateral_balance;
			});
			<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance += collateral_balance);
		} else if collateral_adjustment.is_negative() {
			<Collaterals<T>>::mutate(who, currency_id, |balance| {
				*balance -= collateral_balance;
				// decrease account ref for who when has no amount
				if balance.is_zero() {
					system::Module::<T>::dec_ref(who);
				}
			});
			<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance -= collateral_balance);
		}

		// update debit record
		if debit_adjustment.is_positive() {
			<Debits<T>>::mutate(currency_id, who, |balance| *balance += debit_balance);
			<TotalDebits<T>>::mutate(currency_id, |balance| *balance += debit_balance);
		} else if debit_adjustment.is_negative() {
			<Debits<T>>::mutate(currency_id, who, |balance| *balance -= debit_balance);
			<TotalDebits<T>>::mutate(currency_id, |balance| *balance -= debit_balance);
		}

		Ok(())
	}
}
