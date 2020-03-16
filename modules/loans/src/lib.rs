#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, storage::PrefixIterator, Parameter};
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

const MODULE_ID: ModuleId = ModuleId(*b"aca/vlts");

type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AmountOf<T> = <<T as Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Convert: Convert<(CurrencyIdOf<Self>, Self::DebitBalance), BalanceOf<Self>>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type RiskManager: RiskManager<Self::AccountId, CurrencyIdOf<Self>, AmountOf<Self>, Self::DebitAmount>;
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
	type Treasury: CDPTreasury<Self::AccountId, Balance = BalanceOf<Self>>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Loans {
		pub Debits get(fn debits): double_map hasher(twox_64_concat) CurrencyIdOf<T>, hasher(twox_64_concat) T::AccountId => (T::DebitBalance, Option<(CurrencyIdOf<T>, T::AccountId)>);
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
	{
		/// Update Position success (account, currency_id, collaterals, debits)
		UpdatePosition(AccountId, CurrencyId, Amount, DebitAmount),
		/// Update collaterals and debits success (account, currency_id, collaterals, debits)
		UpdateCollateralsAndDebits(AccountId, CurrencyId, Amount, DebitAmount),
		/// Transfer loan (from, to)
		TransferLoan(AccountId, AccountId, CurrencyId),
	}
);

decl_error! {
	/// Error for loans module.
	pub enum Error for Module<T: Trait> {
		DebitOverflow,
		DebitUnderflow,
		CollateralOverflow,
		CollateralUnderflow,
		AmountIntoBalanceFailed,
		BalanceIntoAmountFailed,
		RiskCheckFailed,
		ExceedDebitValueHardCap,
		CollateralInSufficient,
		AmountIntoDebitBalanceFailed,
		AddBackedDebitFailed,
		SubBackedDebitFailed,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;
	}
}

impl<T: Trait> Module<T> {
	pub fn debits_iterator_with_collateral_prefix(
		currency_id: CurrencyIdOf<T>,
	) -> PrefixIterator<(T::DebitBalance, Option<(CurrencyIdOf<T>, T::AccountId)>)> {
		<Debits<T>>::iter_prefix(currency_id)
	}

	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	// mutate collaterals and debits, don't check position safe and don't mutate token
	pub fn update_collaterals_and_debits(
		who: T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: T::DebitAmount,
	) -> DispatchResult {
		// ensure mutate safe
		Self::check_add_and_sub(&who, currency_id, collaterals, debits)?;
		Self::update_loan(&who, currency_id, collaterals, debits)?;
		Self::deposit_event(RawEvent::UpdateCollateralsAndDebits(
			who,
			currency_id,
			collaterals,
			debits,
		));

		Ok(())
	}

	// mutate collaterals and debits and then mutate stable coin
	pub fn update_position(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: T::DebitAmount,
	) -> DispatchResult {
		// ensure mutate safe
		Self::check_add_and_sub(who, currency_id, collaterals, debits)?;

		// ensure debits cap
		T::RiskManager::check_debit_cap(currency_id, debits).map_err(|_| Error::<T>::ExceedDebitValueHardCap)?;

		// ensure pass risk check
		T::RiskManager::check_position_adjustment(who, currency_id, collaterals, debits)
			.map_err(|_| Error::<T>::RiskCheckFailed)?;

		// ensure account has sufficient balance
		Self::check_balance(who, currency_id, collaterals)?;

		// amount -> balance
		let collateral_balance =
			TryInto::<BalanceOf<T>>::try_into(collaterals.abs()).map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;

		// update stable coin by Treasury
		let debit_balance =
			TryInto::<T::DebitBalance>::try_into(debits.abs()).map_err(|_| Error::<T>::AmountIntoDebitBalanceFailed)?;
		if debits.is_positive() {
			T::Treasury::deposit_backed_debit(who, T::Convert::convert((currency_id, debit_balance)))
				.map_err(|_| Error::<T>::AddBackedDebitFailed)?;
		} else {
			T::Treasury::withdraw_backed_debit(who, T::Convert::convert((currency_id, debit_balance)))
				.map_err(|_| Error::<T>::SubBackedDebitFailed)?;
		}

		let module_account = Self::account_id();
		// update collateral asset
		if collaterals.is_positive() {
			T::Currency::transfer(currency_id, who, &module_account, collateral_balance)
				.expect("Will never fail ensured by check_balance");
		} else {
			T::Currency::transfer(currency_id, &module_account, who, collateral_balance)
				.expect("Will never fail ensured by check_balance");
		}

		// mutate collaterals and debits
		Self::update_loan(who, currency_id, collaterals, debits).expect("Will never fail ensured by check_add_and_sub");

		Self::deposit_event(RawEvent::UpdatePosition(who.clone(), currency_id, collaterals, debits));

		Ok(())
	}

	// transfer loan
	pub fn transfer(from: T::AccountId, to: T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		// get `from` position data
		let collateral: BalanceOf<T> = Self::collaterals(&from, currency_id);
		let (debit, _) = Self::debits(currency_id, &from);

		// balance -> amount
		let collateral: AmountOf<T> =
			TryInto::<AmountOf<T>>::try_into(collateral).map_err(|_| Error::<T>::BalanceIntoAmountFailed)?;
		let debit: T::DebitAmount =
			TryInto::<T::DebitAmount>::try_into(debit).map_err(|_| Error::<T>::BalanceIntoAmountFailed)?;

		// ensure mutate safe
		Self::check_add_and_sub(&from, currency_id, -collateral, -debit)?;
		Self::check_add_and_sub(&to, currency_id, collateral, debit)?;

		// ensure positions passes risk check after transferred
		T::RiskManager::check_position_adjustment(&from, currency_id, -collateral, -debit)
			.map_err(|_| Error::<T>::RiskCheckFailed)?;
		T::RiskManager::check_position_adjustment(&to, currency_id, collateral, debit)
			.map_err(|_| Error::<T>::RiskCheckFailed)?;

		// execute transfer
		Self::update_loan(&from, currency_id, -collateral, -debit)
			.expect("Will never fail ensured by check_add_and_sub");
		Self::update_loan(&to, currency_id, collateral, debit).expect("Will never fail ensured by check_add_and_sub");

		Self::deposit_event(RawEvent::TransferLoan(from, to, currency_id));

		Ok(())
	}

	/// check `who` has sufficient balance
	fn check_balance(who: &T::AccountId, currency_id: CurrencyIdOf<T>, collateral: AmountOf<T>) -> DispatchResult {
		let collaterals_balance =
			TryInto::<BalanceOf<T>>::try_into(collateral.abs()).map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;

		let module_balance = T::Currency::free_balance(currency_id, &Self::account_id());
		let who_balance = T::Currency::free_balance(currency_id, who);

		if collateral.is_positive() {
			ensure!(who_balance >= collaterals_balance, Error::<T>::CollateralInSufficient);
		} else {
			ensure!(
				module_balance >= collaterals_balance,
				Error::<T>::CollateralInSufficient
			);
		}

		Ok(())
	}

	/// ensure sum and sub will success when updating loan collaterals and debits
	fn check_add_and_sub(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: T::DebitAmount,
	) -> DispatchResult {
		// judge collaterals and debits are negative or positive
		let collaterals_balance =
			TryInto::<BalanceOf<T>>::try_into(collaterals.abs()).map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;
		let debits_balance =
			TryInto::<T::DebitBalance>::try_into(debits.abs()).map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;

		// check collaterals update
		if collaterals.is_positive() {
			ensure!(
				Self::total_collaterals(currency_id)
					.checked_add(&collaterals_balance)
					.is_some(),
				Error::<T>::CollateralOverflow
			);
		} else {
			ensure!(
				Self::collaterals(who, currency_id)
					.checked_sub(&collaterals_balance)
					.is_some(),
				Error::<T>::CollateralUnderflow
			);
		}

		// check debits update
		if debits.is_positive() {
			ensure!(
				Self::total_debits(currency_id).checked_add(&debits_balance).is_some(),
				Error::<T>::DebitOverflow
			);
		} else {
			ensure!(
				Self::debits(currency_id, who).0.checked_sub(&debits_balance).is_some(),
				Error::<T>::DebitUnderflow
			);
		}

		Ok(())
	}

	fn update_loan(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: T::DebitAmount,
	) -> DispatchResult {
		// judge collaterals and debits are negative or positive
		let collaterals_balance =
			TryInto::<BalanceOf<T>>::try_into(collaterals.abs()).map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;
		let debits_balance =
			TryInto::<T::DebitBalance>::try_into(debits.abs()).map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;

		// update collaterals record
		if collaterals.is_positive() {
			<Collaterals<T>>::mutate(who, currency_id, |balance| {
				// increase account ref for who when has no amount before
				if balance.is_zero() {
					system::Module::<T>::inc_ref(who);
				}
				*balance += collaterals_balance;
			});
			<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance += collaterals_balance);
		} else {
			<Collaterals<T>>::mutate(who, currency_id, |balance| {
				*balance -= collaterals_balance;
				// decrease account ref for who when has no amount
				if balance.is_zero() {
					system::Module::<T>::dec_ref(who);
				}
			});
			<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance -= collaterals_balance);
		}

		// update debits record
		if debits.is_positive() {
			<Debits<T>>::mutate(currency_id, who, |(balance, key)| {
				if balance.is_zero() {
					*key = Some((currency_id, (*who).clone()));
				}
				*balance += debits_balance;
			});
			<TotalDebits<T>>::mutate(currency_id, |balance| *balance += debits_balance);
		} else {
			<Debits<T>>::mutate(currency_id, who, |(balance, key)| {
				if debits_balance == *balance {
					*key = None;
				}
				*balance -= debits_balance;
			});
			<TotalDebits<T>>::mutate(currency_id, |balance| *balance -= debits_balance);
		}

		Ok(())
	}
}
