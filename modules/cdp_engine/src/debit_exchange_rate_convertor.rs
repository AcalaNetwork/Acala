use super::*;
use sp_runtime::traits::Convert;

pub struct DebitExchangeRateConvertor<T>(marker::PhantomData<T>);

impl<T> Convert<(CurrencyId, Balance), Balance> for DebitExchangeRateConvertor<T>
where
	T: Trait,
{
	fn convert(a: (CurrencyId, Balance)) -> Balance {
		let (currency_id, balance) = a;
		let balance: u128 = balance.unique_saturated_into();
		let balance: Balance = balance.unique_saturated_into();
		<Module<T>>::get_debit_exchange_rate(currency_id).saturating_mul_int(balance)
	}
}
