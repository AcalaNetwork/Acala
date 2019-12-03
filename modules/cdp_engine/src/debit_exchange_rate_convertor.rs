use super::*;
use sp_runtime::traits::Convert;

pub struct DebitExchangeRateConvertor<T>(marker::PhantomData<T>);

impl<T> Convert<(CurrencyIdOf<T>, DebitBalanceOf<T>), BalanceOf<T>> for DebitExchangeRateConvertor<T>
where
	T: Trait,
{
	fn convert(a: (CurrencyIdOf<T>, DebitBalanceOf<T>)) -> BalanceOf<T> {
		let balance = TryInto::<BalanceOf<T>>::try_into(TryInto::<u128>::try_into(a.1).unwrap_or(u128::max_value()))
			.unwrap_or(BalanceOf::<T>::max_value());
		<Module<T>>::debit_exchange_rate(a.0)
			.unwrap_or(T::DefaulDebitExchangeRate::get())
			.checked_mul_int(&balance)
			.unwrap_or(BalanceOf::<T>::max_value())
	}
}
