use crate::{
	AcalaDataProvider, AcalaOracle, CurrencyId, FixedPointNumber, Origin, Price, Runtime, System, TokenSymbol,
};

use frame_support::traits::OnFinalize;
use orml_benchmarking::runtime_benchmarks_instance;
use sp_std::prelude::*;

const MAX_PRICE: u32 = 1000;

runtime_benchmarks_instance! {
	{ Runtime, orml_oracle, AcalaDataProvider }

	_ {
		let u in 1 .. MAX_PRICE => ();
	}

	// feed values with one price
	feed_values {
		let u in ...;
	}: _(Origin::root(), vec![(CurrencyId::Token(TokenSymbol::AUSD), Price::saturating_from_integer(u))])

	// feed values with two price
	feed_values_with_two_price {
		let u in ...;
	}: feed_values(Origin::root(), vec![(CurrencyId::Token(TokenSymbol::AUSD), Price::saturating_from_integer(u)), (CurrencyId::Token(TokenSymbol::ACA), Price::saturating_from_integer(u))])

	// feed values with three price
	feed_values_with_three_price {
		let u in ...;
	}: feed_values(Origin::root(), vec![(CurrencyId::Token(TokenSymbol::AUSD), Price::saturating_from_integer(u)), (CurrencyId::Token(TokenSymbol::ACA), Price::saturating_from_integer(u)), (CurrencyId::Token(TokenSymbol::LDOT), Price::saturating_from_integer(u))])

		on_finalize_with_zero {
			let u in ...;

			System::set_block_number(u);
		}: {
			AcalaOracle::on_finalize(System::block_number());
		}

	on_finalize_with_one {
		let u in ...;

		System::set_block_number(u);
		AcalaOracle::feed_values(Origin::root(), vec![(CurrencyId::Token(TokenSymbol::AUSD), Price::saturating_from_integer(u))])?;
	}: {
		AcalaOracle::on_finalize(System::block_number());
	}

	on_finalize_with_two {
		let u in ...;

		System::set_block_number(u);
		AcalaOracle::feed_values(Origin::root(), vec![(CurrencyId::Token(TokenSymbol::AUSD), Price::saturating_from_integer(u)), (CurrencyId::Token(TokenSymbol::ACA), Price::saturating_from_integer(u))])?;
	}: {
		AcalaOracle::on_finalize(System::block_number());
	}

}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_feed_values() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_feed_values());
		});
	}

	#[test]
	fn test_feed_values_with_two_price() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_feed_values_with_two_price());
		});
	}

	#[test]
	fn test_feed_values_with_three_price() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_feed_values_with_three_price());
		});
	}

	#[test]
	fn test_on_finalize_with_zero() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize_with_zero());
		});
	}

	#[test]
	fn test_on_finalize_with_one() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize_with_one());
		});
	}

	#[test]
	fn test_on_finalize_with_two() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize_with_two());
		});
	}
}
