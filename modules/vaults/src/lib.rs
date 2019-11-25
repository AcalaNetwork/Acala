#![cfg_attr(not(feature = "std"), no_std)]

use codec::FullCodec;
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure};
use orml_traits::{arithmetic::Signed, MultiCurrency, MultiCurrencyExtended, PriceProvider};
use rstd::{convert::TryInto, result};
use sr_primitives::{
	traits::{CheckedAdd, CheckedSub, Convert, Member, SimpleArithmetic, Zero},
	Fixed64,
};

use primitives::CurrencyId as CURRENCY_ID;
use support::RiskManager;

mod mock;
mod tests;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Convert: Convert<(CurrencyIdOf<Self>, DebitBalanceOf<Self>), BalanceOf<Self>>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type DebitCurrency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyIdOf<Self>>;
	type RiskManager: RiskManager<Self::AccountId, CurrencyIdOf<Self>, AmountOf<Self>, DebitAmountOf<Self>>;
}

type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;

type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type DebitBalanceOf<T> = <<T as Trait>::DebitCurrency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;

type AmountOf<T> = <<T as Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;
type DebitAmountOf<T> = <<T as Trait>::DebitCurrency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

decl_storage! {
	trait Store for Module<T: Trait> as Vaults {
		pub Debits get(fn debits): double_map T::AccountId, blake2_256(CurrencyIdOf<T>) => DebitBalanceOf<T>;
		pub Collaterals get(fn collaterals): double_map T::AccountId, blake2_256(CurrencyIdOf<T>) => BalanceOf<T>;
		pub TotalDebits get(fn total_debits): map CurrencyIdOf<T> => DebitBalanceOf<T>;
		pub TotalCollaterals get(fn total_collaterals): map CurrencyIdOf<T> => BalanceOf<T>;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyIdOf<T>,
		DebitAmount = DebitAmountOf<T>,
		Amount = AmountOf<T>,
	{
		/// Update Position success (account, currency_id, collaterals, debits)
		UpdatePosition(AccountId, CurrencyId, Amount, DebitAmount),
		/// Update collateral success (account, currency_id, collaterals, debits)
		UpdateCollateral(AccountId, CurrencyId, Amount, DebitAmount),
		/// Transfer vault (from, to)
		TransferVault(AccountId, AccountId, CurrencyId),
	}
);

decl_error! {
	/// Error for vaults module.
	pub enum Error {
		DebitOverflow,
		CollateralOverflow,
		AmountIntoBalanceFailed,
		BalanceIntoAmountFailed,
		PositionWillUnsafe,
		UpdateStableCoinFailed,
		UpdateCollateralFailed,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;
	}
}

impl<T: Trait> Module<T> {
	pub fn collateral_ratio(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> Option<Fixed64> {
		// ensure debit and collateral exists
		if (true, true)
			!= (
				!<Debits<T>>::exists(&who, currency_id),
				!<Collaterals<T>>::exists(&who, currency_id),
			) {
			return None;
		}

		// get balance of collateral and debits
		let collateral_balance = Self::collaterals(&who, currency_id);
		let debit_balance = Self::debits(&who, currency_id);

		// get stable coin amount
		let stable_balance = T::Convert::convert((currency_id, debit_balance));

		// ensure stable coin balance is not zero
		if stable_balance.is_zero() {
			return None;
		}
		// ensure collateral balance is not zero
		if collateral_balance.is_zero() {
			return None;
		}

		let ausd_currency_id = CURRENCY_ID::AUSD as u8;

		// get prices and calc ratio of collateral
		//if let Some(price) = T::PriceSource::get_price(ausd_currency_id.into(), currency_id) {
		// TODO: fix calculate
		// let result: T::Price = stable_balance.into() / (price * Into::<T::Price>::into(collateral_balance));
		//return Some(Fixed64::from_rational(1i64, 1u64);
		//}

		None
	}

	// mutate collaterlas and debits, don't check position safe and don't mutate stable coin
	pub fn update_collaterals_and_debits(
		who: T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: DebitAmountOf<T>,
	) -> result::Result<(), Error> {
		// ensure mutate safe
		Self::check_add_and_sub(&who, currency_id, collaterals, debits)?;

		Self::update_collateral_and_debit(&who, currency_id, collaterals, debits)?;

		Self::deposit_event(RawEvent::UpdateCollateral(who, currency_id, collaterals, debits));

		Ok(())
	}

	// mulate collaterals and debits and then mulate stable coin
	pub fn update_position(
		who: T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: DebitAmountOf<T>,
	) -> result::Result<(), Error> {
		// ensure mutate safe
		Self::check_add_and_sub(&who, currency_id, collaterals, debits)?;

		// ensure cdp safe
		T::RiskManager::check_position_adjustment(&who, currency_id, collaterals, debits)
			.map_err(|_| Error::PositionWillUnsafe)?;

		// update collateral asset
		T::Currency::update_balance(currency_id, &who, -collaterals).map_err(|_| Error::UpdateCollateralFailed)?;

		// updaet stable coin
		T::DebitCurrency::update_balance(currency_id, &who, debits).map_err(|_| Error::UpdateStableCoinFailed)?;

		// mutate collaterals and debits
		Self::update_collateral_and_debit(&who, currency_id, collaterals, debits)?;

		Self::deposit_event(RawEvent::UpdatePosition(who, currency_id, collaterals, debits));

		Ok(())
	}

	// transfer vault
	pub fn transfer(from: T::AccountId, to: T::AccountId, currency_id: CurrencyIdOf<T>) -> result::Result<(), Error> {
		// get `from` position data
		let collateral: BalanceOf<T> = Self::collaterals(&from, currency_id);
		let debit: DebitBalanceOf<T> = Self::debits(&from, currency_id);

		// banlance -> amount
		let collateral: AmountOf<T> =
			TryInto::<AmountOf<T>>::try_into(collateral).map_err(|_| Error::BalanceIntoAmountFailed)?;
		let debit: DebitAmountOf<T> =
			TryInto::<DebitAmountOf<T>>::try_into(debit).map_err(|_| Error::BalanceIntoAmountFailed)?;

		// ensure mutate safe
		Self::check_add_and_sub(&from, currency_id, -collateral, -debit)?;
		Self::check_add_and_sub(&to, currency_id, collateral, debit)?;

		// ensure positions are safe after transfered
		T::RiskManager::check_position_adjustment(&from, currency_id, -collateral, -debit)
			.map_err(|_| Error::PositionWillUnsafe)?;
		T::RiskManager::check_position_adjustment(&to, currency_id, collateral, debit)
			.map_err(|_| Error::PositionWillUnsafe)?;

		// execute transfer
		Self::update_collateral_and_debit(&from, currency_id, -collateral, -debit)?;
		Self::update_collateral_and_debit(&to, currency_id, collateral, debit)?;

		Self::deposit_event(RawEvent::TransferVault(from, to, currency_id));

		Ok(())
	}

	// ensure sum and sub will success when update
	fn check_add_and_sub(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: DebitAmountOf<T>,
	) -> result::Result<(), Error> {
		// judge collaterals and debits are negative or positive
		let collaterals_balance =
			TryInto::<BalanceOf<T>>::try_into(collaterals.abs()).map_err(|_| Error::AmountIntoBalanceFailed)?;
		let debits_balance =
			TryInto::<DebitBalanceOf<T>>::try_into(debits.abs()).map_err(|_| Error::AmountIntoBalanceFailed)?;

		// check collaterals update
		if collaterals.is_positive() {
			ensure!(
				Self::collaterals(who, currency_id)
					.checked_add(&collaterals_balance)
					.is_some(),
				Error::CollateralOverflow
			);
			ensure!(
				Self::total_collaterals(currency_id)
					.checked_add(&collaterals_balance)
					.is_some(),
				Error::CollateralOverflow
			);
		} else {
			ensure!(
				Self::collaterals(who, currency_id)
					.checked_sub(&collaterals_balance)
					.is_some(),
				Error::CollateralOverflow
			);
			ensure!(
				Self::total_collaterals(currency_id)
					.checked_sub(&collaterals_balance)
					.is_some(),
				Error::CollateralOverflow
			);
		}

		// check collaterals update
		if debits.is_positive() {
			ensure!(
				Self::debits(who, currency_id).checked_add(&debits_balance).is_some(),
				Error::DebitOverflow
			);
			ensure!(
				Self::total_debits(currency_id).checked_add(&debits_balance).is_some(),
				Error::DebitOverflow
			);
		} else {
			ensure!(
				Self::debits(who, currency_id).checked_sub(&debits_balance).is_some(),
				Error::DebitOverflow
			);
			ensure!(
				Self::total_debits(currency_id).checked_sub(&debits_balance).is_some(),
				Error::DebitOverflow
			);
		}

		Ok(())
	}

	fn update_collateral_and_debit(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collaterals: AmountOf<T>,
		debits: DebitAmountOf<T>,
	) -> result::Result<(), Error> {
		// judge collaterals and debits are negative or positive
		let collaterals_balance =
			TryInto::<BalanceOf<T>>::try_into(collaterals.abs()).map_err(|_| Error::AmountIntoBalanceFailed)?;
		let debits_balance =
			TryInto::<DebitBalanceOf<T>>::try_into(debits.abs()).map_err(|_| Error::AmountIntoBalanceFailed)?;

		// updaet collaterals record
		if collaterals.is_positive() {
			<Collaterals<T>>::mutate(who, currency_id, |balance| *balance += collaterals_balance);
			<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance += collaterals_balance);
		} else {
			<Collaterals<T>>::mutate(who, currency_id, |balance| *balance -= collaterals_balance);
			<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance -= collaterals_balance);
		}

		// updaet debits record
		if debits.is_positive() {
			<Debits<T>>::mutate(who, currency_id, |balance| *balance += debits_balance);
			<TotalDebits<T>>::mutate(currency_id, |balance| *balance += debits_balance);
		} else {
			<Debits<T>>::mutate(who, currency_id, |balance| *balance -= debits_balance);
			<TotalDebits<T>>::mutate(currency_id, |balance| *balance -= debits_balance);
		}

		Ok(())
	}
}
