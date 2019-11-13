#![cfg_attr(not(feature = "std"), no_std)]

use codec::FullCodec;
use rstd::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	result,
};
use sr_primitives::traits::{Convert, MaybeSerializeDeserialize, Member, SimpleArithmetic};
use support::{decl_error, decl_module, Parameter};
use traits::{
	arithmetic::{self, Signed},
	BasicCurrency, BasicCurrencyExtended, MultiCurrency, MultiCurrencyExtended,
};

mod mock;
mod tests;

pub type BalanceOf<T> = <<T as Trait>::Currency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;
pub type AmountOf<T> = <<T as Trait>::Currency as BasicCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

pub trait Trait: system::Trait {
	type CurrencyId: FullCodec + Copy + MaybeSerializeDeserialize + Debug;
	type Currency: BasicCurrencyExtended<Self::AccountId>;
	type DebitBalance: Parameter + Member + SimpleArithmetic + Default + Copy + MaybeSerializeDeserialize;
	type Convert: Convert<(Self::CurrencyId, BalanceOf<Self>), Self::DebitBalance>
		+ Convert<(Self::CurrencyId, Self::DebitBalance), BalanceOf<Self>>;
	type DebitAmount: Signed
		+ TryInto<Self::DebitBalance>
		+ TryFrom<Self::DebitBalance>
		+ Parameter
		+ Member
		+ arithmetic::SimpleArithmetic
		+ Default
		+ Copy
		+ MaybeSerializeDeserialize;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {}
}

decl_error! {
	/// Error for debit module.
	pub enum Error {
		DebitDepositFailed,
		DebitWithdrawFailed,
		AmountIntoDebitBalanceFailed,
	}
}

impl<T: Trait> Module<T> {}

impl<T: Trait> MultiCurrency<T::AccountId> for Module<T> {
	type Balance = T::DebitBalance;
	type CurrencyId = T::CurrencyId;
	type Error = Error;

	// be of no effect
	fn total_issuance(_currency_id: Self::CurrencyId) -> Self::Balance {
		Self::Balance::default()
	}

	// be of no effect
	fn balance(_currency_id: Self::CurrencyId, _who: &T::AccountId) -> Self::Balance {
		Self::Balance::default()
	}

	// be of no effect
	fn transfer(
		_currency_id: Self::CurrencyId,
		_from: &T::AccountId,
		_to: &T::AccountId,
		_amount: Self::Balance,
	) -> result::Result<(), Self::Error> {
		Ok(())
	}

	fn deposit(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		debit_amount: Self::Balance,
	) -> result::Result<(), Self::Error> {
		let stable_coin_amount: BalanceOf<T> = T::Convert::convert((currency_id, debit_amount));
		T::Currency::deposit(who, stable_coin_amount).map_err(|_| Error::DebitDepositFailed)
	}

	fn withdraw(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		debit_amount: Self::Balance,
	) -> result::Result<(), Self::Error> {
		let stable_coin_amount: BalanceOf<T> = T::Convert::convert((currency_id, debit_amount));
		T::Currency::withdraw(who, stable_coin_amount).map_err(|_| Error::DebitWithdrawFailed)
	}

	// be of no effect
	fn slash(_currency_id: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> Self::Balance {
		Self::Balance::default()
	}
}

impl<T: Trait> MultiCurrencyExtended<T::AccountId> for Module<T> {
	type Amount = T::DebitAmount;

	fn update_balance(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		debit_amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let debit_balance =
			TryInto::<Self::Balance>::try_into(debit_amount.abs()).map_err(|_| Error::AmountIntoDebitBalanceFailed)?;
		if debit_amount.is_positive() {
			Self::deposit(currency_id, who, debit_balance)
		} else {
			Self::withdraw(currency_id, who, debit_balance)
		}
	}
}
