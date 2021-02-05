#![cfg(test)]
use super::*;
use crate::precompile::mock::{
	alice, bob, new_test_ext, run_to_block, Balances, Oracle, OraclePrecompile, Price, ScheduleCallPrecompile, Test,
	ACA_ERC20_ADDRESS, ALICE, XBTC,
};
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use module_evm::ExitError;
use orml_traits::DataFeeder;
use primitives::{evm::AddressMapping, PREDEPLOY_ADDRESS_START};
use sp_core::{H160, U256};
use sp_runtime::FixedPointNumber;

pub struct DummyPrecompile;
impl Precompile for DummyPrecompile {
	fn execute(
		_input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		Ok((ExitSucceed::Stopped, vec![], 0))
	}
}

pub type WithSystemContractFilter = AllPrecompiles<
	crate::SystemContractsFilter,
	DummyPrecompile,
	DummyPrecompile,
	DummyPrecompile,
	DummyPrecompile,
	DummyPrecompile,
>;

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
		let (reason, output, used_gas) = OraclePrecompile::execute(&input, None, &context).unwrap();
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, [0u8; 64]);
		assert_eq!(used_gas, 0);

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

#[test]
fn schedule_call_precompile_should_work() {
	new_test_ext().execute_with(|| {
		let context = Context {
			address: Default::default(),
			caller: alice(),
			apparent_value: Default::default(),
		};

		let mut input = [0u8; 10 * 32 + 4];
		// action
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// from
		U256::from(alice().as_bytes()).to_big_endian(&mut input[1 * 32..2 * 32]);
		// target
		U256::from(ACA_ERC20_ADDRESS).to_big_endian(&mut input[2 * 32..3 * 32]);
		// value
		U256::from(0).to_big_endian(&mut input[3 * 32..4 * 32]);
		// gas_limit
		U256::from(300000).to_big_endian(&mut input[4 * 32..5 * 32]);
		// storage_limit
		U256::from(100).to_big_endian(&mut input[5 * 32..6 * 32]);
		// min_delay
		U256::from(1).to_big_endian(&mut input[6 * 32..7 * 32]);
		// input_len
		U256::from(4 + 32 + 32).to_big_endian(&mut input[7 * 32..8 * 32]);

		// input_data
		let mut transfer_to_bob = [0u8; 68];
		// transfer bytes4(keccak256(signature)) 0xa9059cbb
		transfer_to_bob[0..4].copy_from_slice(&hex!("a9059cbb"));
		// to address
		U256::from(bob().as_bytes()).to_big_endian(&mut transfer_to_bob[4..36]);
		// amount
		U256::from(1000).to_big_endian(&mut transfer_to_bob[36..68]);

		U256::from(&transfer_to_bob[0..32]).to_big_endian(&mut input[8 * 32..9 * 32]);
		U256::from(&transfer_to_bob[32..64]).to_big_endian(&mut input[9 * 32..10 * 32]);
		input[10 * 32..10 * 32 + 4].copy_from_slice(&transfer_to_bob[64..68]);

		let (reason, output, used_gas) = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();

		// returned block + index
		let mut expected_output = [0u8; 64];
		U256::from(3).to_big_endian(&mut expected_output[..32]);
		U256::from(0).to_big_endian(&mut expected_output[32..64]);
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		let from_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&alice());
		let to_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&bob());
		#[cfg(not(feature = "with-ethereum-compatibility"))]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 999999700000);
			assert_eq!(Balances::reserved_balance(from_account.clone()), 300000);
			assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
		}
		#[cfg(feature = "with-ethereum-compatibility")]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 1000000000000);
			assert_eq!(Balances::reserved_balance(from_account.clone()), 0);
			assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
		}

		run_to_block(4);
		#[cfg(not(feature = "with-ethereum-compatibility"))]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 999999995255);
			assert_eq!(Balances::reserved_balance(from_account), 0);
			assert_eq!(Balances::free_balance(to_account), 1000000001000);
		}
		#[cfg(feature = "with-ethereum-compatibility")]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 999999995255);
			assert_eq!(Balances::reserved_balance(from_account), 0);
			assert_eq!(Balances::free_balance(to_account), 1000000001000);
		}
	});
}

#[test]
fn schedule_call_precompile_should_handle_invalid_input() {
	new_test_ext().execute_with(|| {
		let context = Context {
			address: Default::default(),
			caller: alice(),
			apparent_value: Default::default(),
		};

		let mut input = [0u8; 8 * 32 + 1];
		// action
		U256::default().to_big_endian(&mut input[0 * 32..1 * 32]);
		// from
		U256::from(alice().as_bytes()).to_big_endian(&mut input[1 * 32..2 * 32]);
		// target
		U256::from(ACA_ERC20_ADDRESS).to_big_endian(&mut input[2 * 32..3 * 32]);
		// value
		U256::from(0).to_big_endian(&mut input[3 * 32..4 * 32]);
		// gas_limit
		U256::from(300000).to_big_endian(&mut input[4 * 32..5 * 32]);
		// storage_limit
		U256::from(100).to_big_endian(&mut input[5 * 32..6 * 32]);
		// min_delay
		U256::from(1).to_big_endian(&mut input[6 * 32..7 * 32]);
		// input_len
		U256::from(1).to_big_endian(&mut input[7 * 32..8 * 32]);

		// input_data = 0x12
		input[8 * 32] = hex!("12")[0];

		let (reason, output, used_gas) = ScheduleCallPrecompile::execute(&input, None, &context).unwrap();

		// returned block + index
		let mut expected_output = [0u8; 64];
		U256::from(3).to_big_endian(&mut expected_output[..32]);
		U256::from(0).to_big_endian(&mut expected_output[32..64]);
		assert_eq!(reason, ExitSucceed::Returned);
		assert_eq!(output, expected_output);
		assert_eq!(used_gas, 0);

		let from_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&alice());
		let to_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&bob());
		#[cfg(not(feature = "with-ethereum-compatibility"))]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 999999700000);
			assert_eq!(Balances::reserved_balance(from_account.clone()), 300000);
			assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
		}
		#[cfg(feature = "with-ethereum-compatibility")]
		{
			assert_eq!(Balances::free_balance(from_account.clone()), 1000000000000);
			assert_eq!(Balances::reserved_balance(from_account.clone()), 0);
			assert_eq!(Balances::free_balance(to_account.clone()), 1000000000000);
		}

		run_to_block(4);
		assert_eq!(Balances::free_balance(from_account.clone()), 999999999954);
		assert_eq!(Balances::reserved_balance(from_account), 0);
		assert_eq!(Balances::free_balance(to_account), 1000000000000);
	});
}
