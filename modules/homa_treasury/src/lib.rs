#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, traits::Get};
use frame_system::{self as system};
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId, EraIndex};
use sp_runtime::{traits::AccountIdConversion, ModuleId};
use support::{HomaProtocol, OnCommission};

pub trait Trait: system::Trait {
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	type Homa: HomaProtocol<Self::AccountId, Balance, EraIndex>;
	type StakingCurrencyId: Get<CurrencyId>;

	/// The Homa treasury's module id, keep benefits from Homa protocol.
	type ModuleId: Get<ModuleId>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		const StakingCurrencyId: CurrencyId = T::StakingCurrencyId::get();
		const ModuleId: ModuleId = T::ModuleId::get();
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
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
