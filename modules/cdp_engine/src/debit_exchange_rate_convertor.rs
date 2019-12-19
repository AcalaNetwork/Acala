use super::*;
use sp_runtime::traits::Convert;

pub struct DebitExchangeRateConvertor<T>(marker::PhantomData<T>);

impl<T> Convert<(CurrencyIdOf<T>, DebitBalanceOf<T>), BalanceOf<T>> for DebitExchangeRateConvertor<T>
where
	T: Trait,
{
	fn convert(a: (CurrencyIdOf<T>, DebitBalanceOf<T>)) -> BalanceOf<T> {
		let (currency_id, balance) = a;
		let balance: u128 = balance.unique_saturated_into();
		let balance: BalanceOf<T> = balance.unique_saturated_into();
		<Module<T>>::debit_exchange_rate(currency_id)
			.unwrap_or(T::DefaulDebitExchangeRate::get())
			.saturating_mul_int(&balance)
	}
}
