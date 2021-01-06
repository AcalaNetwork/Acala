use super::utils::{dollars, set_balance};
use crate::{AccountId, Currencies, GetStakingCurrencyId, Homa, PolkadotBondingDuration, PolkadotBridge, Runtime};
use frame_benchmarking::account;
use frame_system::RawOrigin;
use module_homa::RedeemStrategy;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_homa }

	_ {}

	// inject DOT to staking pool and mint LDOT
	mint {
		let caller: AccountId = account("caller", 0, SEED);
		set_balance(GetStakingCurrencyId::get(), &caller, dollars(1_000u128));
	}: _(RawOrigin::Signed(caller), dollars(1_000u128))

	// redeem DOT from free pool
	redeem_immediately {
		let caller: AccountId = account("caller", 0, SEED);
		set_balance(GetStakingCurrencyId::get(), &caller, dollars(1_000u128));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), dollars(1_000u128))?;
		for era_index in 0..=PolkadotBondingDuration::get() {
			PolkadotBridge::new_era(Default::default());
		}
	}: redeem(RawOrigin::Signed(caller.clone()), dollars(1u128), RedeemStrategy::Immediately)
	verify {
		assert!(<Currencies as MultiCurrency<_>>::total_balance(GetStakingCurrencyId::get(), &caller) > 0);
	}

	// redeem DOT by wait for complete unbonding eras
	redeem_wait_for_unbonding {
		let caller: AccountId = account("caller", 0, SEED);
		set_balance(GetStakingCurrencyId::get(), &caller, dollars(1_000u128));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), dollars(1_000u128))?;
		PolkadotBridge::new_era(Default::default());
	}: redeem(RawOrigin::Signed(caller), dollars(1u128), RedeemStrategy::WaitForUnbonding)

	// redeem DOT by claim unbonding
	redeem_by_claim_unbonding {
		let caller: AccountId = account("caller", 0, SEED);
		set_balance(GetStakingCurrencyId::get(), &caller, dollars(1_000u128));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), dollars(1_000u128))?;
		PolkadotBridge::new_era(Default::default());
		PolkadotBridge::new_era(Default::default());
	}: redeem(RawOrigin::Signed(caller.clone()), dollars(1u128), RedeemStrategy::Target(PolkadotBondingDuration::get() + 2))

	withdraw_redemption {
		let caller: AccountId = account("caller", 0, SEED);
		set_balance(GetStakingCurrencyId::get(), &caller, dollars(1_000u128));
		Homa::mint(RawOrigin::Signed(caller.clone()).into(), dollars(1_000u128))?;
		PolkadotBridge::new_era(Default::default());
		Homa::redeem(RawOrigin::Signed(caller.clone()).into(), dollars(1u128), RedeemStrategy::WaitForUnbonding)?;
		for era_index in 0..=PolkadotBondingDuration::get() {
			PolkadotBridge::new_era(Default::default());
		}
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert!(<Currencies as MultiCurrency<_>>::total_balance(GetStakingCurrencyId::get(), &caller) > 0);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;
	use sp_runtime::{FixedPointNumber, FixedU128};

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		module_staking_pool::GenesisConfig {
			staking_pool_params: module_staking_pool::Params {
				target_max_free_unbonded_ratio: FixedU128::saturating_from_rational(10, 100),
				target_min_free_unbonded_ratio: FixedU128::saturating_from_rational(5, 100),
				target_unbonding_to_free_ratio: FixedU128::saturating_from_rational(2, 100),
				unbonding_to_free_adjustment: FixedU128::saturating_from_rational(1, 1000),
				base_fee_rate: FixedU128::saturating_from_rational(2, 100),
			},
		}
		.assimilate_storage(&mut t)
		.unwrap();
		t.into()
	}

	#[test]
	fn test_mint() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_mint());
		});
	}

	#[test]
	fn test_redeem_immediately() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_redeem_immediately());
		});
	}

	#[test]
	fn test_redeem_wait_for_unbonding() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_redeem_wait_for_unbonding());
		});
	}

	#[test]
	fn test_redeem_by_claim_unbonding() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_redeem_by_claim_unbonding());
		});
	}

	#[test]
	fn test_withdraw_redemption() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_withdraw_redemption());
		});
	}
}
