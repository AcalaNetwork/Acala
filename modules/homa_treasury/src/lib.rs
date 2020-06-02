#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, traits::Get};
use frame_system::{self as system};
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId, EraIndex};
use sp_runtime::{traits::AccountIdConversion, ModuleId};
use support::{HomaProtocol, OnCommission};

const MODULE_ID: ModuleId = ModuleId(*b"aca/hmts");

pub trait Trait: system::Trait {
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	type Homa: HomaProtocol<Self::AccountId, Balance, EraIndex>;
	type StakingCurrencyId: Get<CurrencyId>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		const StakingCurrencyId: CurrencyId = T::StakingCurrencyId::get();
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}
}

impl<T: Trait> OnCommission<Balance, CurrencyId> for Module<T> {
	fn on_commission(currency_id: CurrencyId, amount: Balance) {
		let module_account = Self::account_id();
		if T::Currency::deposit(currency_id, &module_account, amount).is_ok()
			&& currency_id == T::StakingCurrencyId::get()
		{
			if let Ok(_amount) = T::Homa::mint(&module_account, amount) {}
		}
	}
}
