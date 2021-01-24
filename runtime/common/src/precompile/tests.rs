#![cfg(test)]
use super::*;
use crate::precompile::mock::{alice, new_test_ext, Oracle, OraclePrecompile, Price, ALICE, XBTC};
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use module_evm::ExitError;
use orml_traits::DataFeeder;
use primitives::PREDEPLOY_ADDRESS_START;
use sp_core::U256;
use sp_runtime::FixedPointNumber;

pub struct DummyPrecompile;
impl Precompile for DummyPrecompile {
	fn execute(
		_input: &[u8],
		_target_gas: Option<usize>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		Ok((ExitSucceed::Stopped, vec![], 0))
	}
}

pub type WithSystemContractFilter =
	AllPrecompiles<crate::SystemContractsFilter, DummyPrecompile, DummyPrecompile, DummyPrecompile, DummyPrecompile>;

#[test]
fn precompile_filter_works_on_acala_precompiles() {
	let precompile = H160::from_low_u64_be(PRECOMPILE_ADDRESS_START);

	let mut non_system = [0u8; 20];
	non_system[0] = 1;

	let non_system_caller_context = Context {
		address: precompile,
		caller: non_system.into(),
		apparent_value: 0.into(),
	};
	assert_eq!(
		WithSystemContractFilter::execute(precompile, &[0u8; 1], None, &non_system_caller_context),
		Some(Err(ExitError::Other("no permission".into()))),
	);
}

#[test]
fn precompile_filter_does_not_work_on_system_contracts() {
	let system = H160::from_low_u64_be(PREDEPLOY_ADDRESS_START);

	let mut non_system = [0u8; 20];
	non_system[0] = 1;

	let non_system_caller_context = Context {
		address: system,
		caller: non_system.into(),
		apparent_value: 0.into(),
	};
	assert!(
		WithSystemContractFilter::execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context).is_none()
	);
}

#[test]
fn precompile_filter_does_not_work_on_non_system_contracts() {
	let mut non_system = [0u8; 20];
	non_system[0] = 1;
	let mut another_non_system = [0u8; 20];
	another_non_system[0] = 2;

	let non_system_caller_context = Context {
		address: non_system.into(),
		caller: another_non_system.into(),
		apparent_value: 0.into(),
	};
	assert!(
		WithSystemContractFilter::execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context).is_none()
	);
}

#[test]
fn oracle_precompile_should_work() {
	new_test_ext().execute_with(|| {
		let context = Context {
			address: Default::default(),
			caller: alice(),
			apparent_value: Default::default(),
		};

		let price = Price::from(30_000);

		// action + currency_id
		let mut input = [0u8; 64];
		U256::default().to_big_endian(&mut input[..32]);
		U256::from_big_endian(&hex!("0300").to_vec()).to_big_endian(&mut input[32..64]);

		// no price yet
		assert_noop!(
			OraclePrecompile::execute(&input, None, &context),
			ExitError::Other("no data".into())
		);

		assert_ok!(Oracle::feed_value(ALICE, XBTC, price));
		assert_eq!(
			Oracle::get_no_op(&XBTC),
			Some(orml_oracle::TimestampedValue {
				value: price,
				timestamp: 1
			})
		);

		// returned price + timestamp
		let mut expected_output = [0u8; 64];
		U256::from(price.into_inner()).to_big_endian(&mut expected_output[..32]);
		U256::from(1).to_big_endian(&mut expected_output[32..64]);

		let (reason, output, used_gas) = OraclePrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);
	});
}

#[test]
fn oracle_precompile_should_handle_invalid_input() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			OraclePrecompile::execute(
				&[0u8; 0],
				None,
				&Context {
					address: Default::default(),
					caller: alice(),
					apparent_value: Default::default()
				}
			),
			ExitError::Other("invalid input".into())
		);

		assert_noop!(
			OraclePrecompile::execute(
				&[0u8; 32],
				None,
				&Context {
					address: Default::default(),
					caller: alice(),
					apparent_value: Default::default()
				}
			),
			ExitError::Other("invalid input".into())
		);

		assert_noop!(
			OraclePrecompile::execute(
				&[1u8; 32],
				None,
				&Context {
					address: Default::default(),
					caller: alice(),
					apparent_value: Default::default()
				}
			),
			ExitError::Other("unknown action".into())
		);
	});
}
