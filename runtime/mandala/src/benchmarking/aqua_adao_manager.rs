use super::utils::{dollar, feed_price, set_balance};
use crate::*;

use frame_benchmarking::whitelisted_caller;
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;
use sp_runtime::{traits::One, FixedI128, FixedU128};

use primitives::currency::DexShare;

use ecosystem_aqua_adao_manager::{Allocation, AllocationAdjustment, Strategy, StrategyKind};

const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const ADAO_AUSD_LP: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::AUSD), DexShare::Token(TokenSymbol::ADAO));
const ADAO: CurrencyId = CurrencyId::Token(TokenSymbol::ADAO);

const MAX_ALLOCATIONS_COUNT: u32 = 20;

runtime_benchmarks! {
	{ Runtime, ecosystem_aqua_adao_manager }

	set_target_allocations {
		let n in 1 .. MAX_ALLOCATIONS_COUNT;

		let allocation = Allocation {
			value: dollar(STABLECOIN) * 1_000_000,
			range: dollar(STABLECOIN) * 100_000,
		};
		let mut allocations = Vec::new();
		for i in 0..n {
			let asset_id: u16 = i.saturated_into();
			allocations.push((CurrencyId::ForeignAsset(asset_id), Some(allocation.clone())));
		}
	}: _(RawOrigin::Root, allocations)

	adjust_target_allocations {
		let n in 1 .. MAX_ALLOCATIONS_COUNT;

		let allocation = Allocation {
			value: dollar(STABLECOIN) * 1_000_000,
			range: dollar(STABLECOIN) * 100_000,
		};
		let mut allocations = Vec::new();
		for i in 1..MAX_ALLOCATIONS_COUNT {
			let asset_id: u16 = i.saturated_into();
			allocations.push((CurrencyId::ForeignAsset(asset_id), Some(allocation.clone())));
		}
		AquaAdaoManager::set_target_allocations(RawOrigin::Root.into(), allocations)?;

		let adjustment = AllocationAdjustment {
			value: 1_000_000_000_000_000,
			range: -1_000_000_000_000_000,
		};
		let mut adjustments = Vec::new();
		for i in 1..n {
			let asset_id: u16 = i.saturated_into();
			adjustments.push((CurrencyId::ForeignAsset(asset_id), adjustment.clone()));
		}
	}: _(RawOrigin::Root, adjustments)

	set_strategies {
		let strategy = Strategy {
			kind: StrategyKind::LiquidityProvisionAusdAdao,
			percent_per_trade: One::one(),
			max_amount_per_trade: 1_000_000_000_000_000_000,
			min_amount_per_trade: 1_000_000_000_000_000,
		};
		let strategies = vec![strategy.clone(), strategy.clone(), strategy.clone(), strategy];
	}: _(RawOrigin::Root, strategies)

	on_initialize_with_rebalance {
		let alice = whitelisted_caller();
		set_balance(STABLECOIN, &alice, dollar(STABLECOIN) * 1_000_000);
		set_balance(ADAO, &alice, dollar(STABLECOIN) * 1_000_000);
		Dex::add_liquidity(
			Origin::signed(AccountId::from(alice.clone())),
			ADAO,
			STABLECOIN,
			1_000 * dollar(ADAO),
			1_000 * dollar(STABLECOIN),
			0,
			false,
		)?;
		DexOracle::enable_average_price(
			Origin::root(),
			ADAO,
			STABLECOIN,
			1
		)?;
		DexOracle::on_initialize(1);

		set_balance(STABLECOIN, &DaoAccount::get(), dollar(STABLECOIN) * 1_000_000);
		feed_price(vec![(STABLECOIN, One::one()), (ADAO, One::one()), (ADAO_AUSD_LP, One::one())])?;

		// set allocations
		let allocation = Allocation { value: dollar(STABLECOIN) * 100, range: dollar(STABLECOIN) * 10 };
		AquaAdaoManager::set_target_allocations(
			RawOrigin::Root.into(),
			vec![(STABLECOIN, Some(allocation)), (ADAO_AUSD_LP, Some(allocation))]
		)?;

		// set strategy
		let strategy = Strategy {
			kind: StrategyKind::LiquidityProvisionAusdAdao,
			percent_per_trade: FixedU128::saturating_from_rational(1, 2),
			max_amount_per_trade: 1_000_000_000_000_000_000,
			min_amount_per_trade: -1_000_000_000_000,
		};
		AquaAdaoManager::set_strategies(RawOrigin::Root.into(), vec![strategy])?;
	}: {
		AquaAdaoManager::on_initialize(11)
	}

	on_initialize_without_rebalance {}: {
		AquaAdaoManager::on_initialize(2)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}