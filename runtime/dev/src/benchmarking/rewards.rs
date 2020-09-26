use crate::{AccountId, CurrencyId, Rewards, Runtime, System, TokenSymbol};

use frame_benchmarking::account;
use frame_support::storage::StorageMap;
use frame_support::traits::OnInitialize;
use module_incentives::PoolId;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

const SEED: u32 = 0;
const MAX_USER_INDEX: u32 = 1000;
const MAX_BLOCK_NUMBER: u32 = 1000;

runtime_benchmarks! {
	{ Runtime, orml_rewards }

	_ {
		let u in 1 .. MAX_USER_INDEX => ();
		let p in 1 .. MAX_BLOCK_NUMBER => ();
		let b in 1 .. MAX_BLOCK_NUMBER => ();
	}

	on_initialize_with_one {
		let u in ...;
		let p in ...;
		let b in ...;
		let who: AccountId = account("who", u, SEED);
		let pool_id = PoolId::Loans(CurrencyId::Token(TokenSymbol::DOT));

		orml_rewards::Pools::<Runtime>::mutate(pool_id, |pool_info| {
			pool_info.total_rewards += 100;
		});
		Rewards::on_initialize(1);
		System::set_block_number(b);
	}: {
		Rewards::on_initialize(System::block_number());
	}

	on_initialize_with_two {
		let u in ...;
		let p in ...;
		let b in ...;
		let who: AccountId = account("who", u, SEED);
		let pool_id_1 = PoolId::Loans(CurrencyId::Token(TokenSymbol::DOT));
		let pool_id_2 = PoolId::Loans(CurrencyId::Token(TokenSymbol::AUSD));

		orml_rewards::Pools::<Runtime>::mutate(pool_id_1, |pool_info| {
			pool_info.total_rewards += 100;
		});
		orml_rewards::Pools::<Runtime>::mutate(pool_id_2, |pool_info| {
			pool_info.total_rewards += 200;
		});
		Rewards::on_initialize(1);
		System::set_block_number(b);
	}: {
		Rewards::on_initialize(System::block_number());
	}

	on_initialize_with_three {
		let u in ...;
		let p in ...;
		let b in ...;
		let who: AccountId = account("who", u, SEED);
		let pool_id_1 = PoolId::Loans(CurrencyId::Token(TokenSymbol::DOT));
		let pool_id_2 = PoolId::Loans(CurrencyId::Token(TokenSymbol::AUSD));
		let pool_id_3 = PoolId::Loans(CurrencyId::Token(TokenSymbol::XBTC));

		orml_rewards::Pools::<Runtime>::mutate(pool_id_1, |pool_info| {
			pool_info.total_rewards += 100;
		});
		orml_rewards::Pools::<Runtime>::mutate(pool_id_2, |pool_info| {
			pool_info.total_rewards += 200;
		});
		orml_rewards::Pools::<Runtime>::mutate(pool_id_3, |pool_info| {
			pool_info.total_rewards += 300;
		});
		Rewards::on_initialize(1);
		System::set_block_number(b);
	}: {
		Rewards::on_initialize(System::block_number());
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
	fn test_on_initialize_with_one() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_initialize_with_one());
		});
	}

	#[test]
	fn test_on_initialize_with_two() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_initialize_with_two());
		});
	}

	#[test]
	fn test_on_initialize_with_three() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_initialize_with_three());
		});
	}
}
