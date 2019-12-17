#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_storage, traits::Get};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, Saturating},
	ModuleId,
};
use support::CDPTreasury;

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/trsy");

type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;

pub trait Trait: system::Trait {
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
}

decl_storage! {
	trait Store for Module<T: Trait> as CDPTreasury {
		DebitPool get(fn debit_pool): BalanceOf<T>;
		SurplusPool get(fn surplus_pool): BalanceOf<T>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn on_finalize(_now: T::BlockNumber) {
			let amount = rstd::cmp::min(Self::debit_pool(), Self::surplus_pool());
			if amount > 0.into() {
				if T::Currency::withdraw(T::GetStableCurrencyId::get(), &Self::account_id(), amount).is_ok() {
					<DebitPool<T>>::mutate(|debit| *debit -= amount);
					<SurplusPool<T>>::mutate(|surplus| *surplus -= amount);
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}
}

impl<T: Trait> CDPTreasury for Module<T> {
	type Balance = BalanceOf<T>;

	fn on_debit(amount: Self::Balance) {
		<DebitPool<T>>::mutate(|debit| *debit = debit.saturating_add(amount));
	}

	fn on_surplus(amount: Self::Balance) {
		if T::Currency::balance(T::GetStableCurrencyId::get(), &Self::account_id())
			.checked_add(&amount)
			.is_some()
		{
			T::Currency::deposit(T::GetStableCurrencyId::get(), &Self::account_id(), amount)
				.expect("never failed after overflow check");
			<SurplusPool<T>>::mutate(|surplus| *surplus += amount);
		}
	}
}
