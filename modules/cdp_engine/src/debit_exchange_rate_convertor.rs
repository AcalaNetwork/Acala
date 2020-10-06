use super::*;
use sp_runtime::traits::Convert;

pub struct DebitExchangeRateConvertor<T>(marker::PhantomData<T>);

impl<T> Convert<(CurrencyId, Balance), Balance> for DebitExchangeRateConvertor<T>
where
	T: Trait,
{
	fn convert((currency_id, balance): (CurrencyId, Balance)) -> Balance {
		<Module<T>>::get_debit_exchange_rate(currency_id).saturating_mul_int(balance)
	}
}
