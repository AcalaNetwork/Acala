use super::*;
use primitives::{Balance, CurrencyId};
use sp_runtime::traits::Convert;
use sp_runtime::FixedPointNumber;

pub struct DebitExchangeRateConvertor<T>(sp_std::marker::PhantomData<T>);

impl<T> Convert<(CurrencyId, Balance), Balance> for DebitExchangeRateConvertor<T>
where
	T: Config,
{
	fn convert((currency_id, balance): (CurrencyId, Balance)) -> Balance {
		<Module<T>>::get_debit_exchange_rate(currency_id).saturating_mul_int(balance)
	}
}
