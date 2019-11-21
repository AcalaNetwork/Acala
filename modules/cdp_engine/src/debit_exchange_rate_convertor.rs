#![cfg_attr(not(feature = "std"), no_std)]

use super::*;
use sr_primitives::traits::Convert;

pub struct DebitExchangeRateConvertor<T>(marker::PhantomData<T>);

impl<T> Convert<(CurrencyIdOf<T>, BalanceOf<T>), DebitBalanceOf<T>> for DebitExchangeRateConvertor<T>
where
	T: Trait,
{
	fn convert(a: (CurrencyIdOf<T>, BalanceOf<T>)) -> DebitBalanceOf<T> {
		TryInto::<DebitBalanceOf<T>>::try_into(
			a.1.saturated_into::<u128>() * TryInto::<u128>::try_into(Fixed64::accuracy()).unwrap_or(U128_BILLION)
				/ TryInto::<u128>::try_into(T::debit_exchange_rate(a.0).unwrap_or(Fixed64::accuracy()))
					.unwrap_or(U128_BILLION),
		)
		.unwrap_or(0.into())
	}
}

// impl<T> Convert<(CurrencyIdOf<T>, DebitBalanceOf<T>), BalanceOf<T>> for DebitExchangeRateConvertor<T>
// where
// 	T: Trait,
// {
// 	fn convert(a: (CurrencyIdOf<T>, DebitBalanceOf<T>)) -> BalanceOf<T> {
// 		TryInto::<BalanceOf<T>>::try_into(
// 			a.1.saturated_into::<u128>()
// 				* TryInto::<u128>::try_into(T::debit_exchange_rate(a.0).unwrap_or(Fixed64::accuracy()))
// 					.unwrap_or(U128_BILLION)
// 				/ TryInto::<u128>::try_into(Fixed64::accuracy()).unwrap_or(U128_BILLION),
// 		)
// 		.unwrap_or(0.into())
// 	}
// }
