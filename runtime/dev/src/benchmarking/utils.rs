use crate::{
	AccountId, Accounts, Balance, Currencies, CurrencyId, GetNativeCurrencyId, NewAccountDeposit, Runtime, TokenSymbol,
	DOLLARS,
};

use frame_support::traits::StoredMap;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::traits::{SaturatedConversion, StaticLookup};

pub fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Trait>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Trait>::Lookup::unlookup(who)
}

pub fn set_balance(currency_id: CurrencyId, who: &AccountId, balance: Balance) {
	if !Accounts::is_explicit(who) {
		let _ = <Currencies as MultiCurrencyExtended<_>>::update_balance(
			GetNativeCurrencyId::get(),
			&who,
			NewAccountDeposit::get().saturated_into(),
		);
	}
	let _ = <Currencies as MultiCurrencyExtended<_>>::update_balance(currency_id, &who, balance.saturated_into());
	assert_eq!(
		<Currencies as MultiCurrency<_>>::free_balance(currency_id, who),
		balance
	);
}

pub fn set_ausd_balance(who: &AccountId, balance: Balance) {
	set_balance(CurrencyId::Token(TokenSymbol::AUSD), who, balance)
}

pub fn set_aca_balance(who: &AccountId, balance: Balance) {
	set_balance(CurrencyId::Token(TokenSymbol::ACA), who, balance)
}

pub fn dollars<T: Into<u128>>(d: T) -> Balance {
	DOLLARS.saturating_mul(d.into())
}
