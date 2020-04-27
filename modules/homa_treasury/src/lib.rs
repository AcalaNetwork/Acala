#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, traits::Get};
use orml_traits::MultiCurrency;
use sp_runtime::{traits::AccountIdConversion, ModuleId};
use support::{HomaProtocol, OnCommission};

const MODULE_ID: ModuleId = ModuleId(*b"aca/hmts");

type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Currency: MultiCurrency<Self::AccountId>;
	type Homa: HomaProtocol<Self::AccountId, Balance = BalanceOf<Self>>;
	type StakingCurrencyId: Get<CurrencyIdOf<Self>>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		const StakingCurrencyId: CurrencyIdOf<T> = T::StakingCurrencyId::get();
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}
}

impl<T: Trait> OnCommission<BalanceOf<T>, CurrencyIdOf<T>> for Module<T> {
	fn on_commission(currency_id: CurrencyIdOf<T>, amount: BalanceOf<T>) {
		let module_account = Self::account_id();
		if T::Currency::deposit(currency_id, &module_account, amount).is_ok() {
			if currency_id == T::StakingCurrencyId::get() {
				if let Ok(_amount) = T::Homa::mint(&module_account, amount) {}
			}
		}
	}
}
