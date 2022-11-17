// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

mod relay_chain;

#[cfg(feature = "with-karura-runtime")]
mod erc20;
#[cfg(feature = "with-karura-runtime")]
mod kusama_cross_chain_transfer;
#[cfg(feature = "with-karura-runtime")]
pub mod kusama_test_net;
// NOTE: Unsupport now
// https://github.com/paritytech/cumulus/blob/26e17fea7ef95d91b94db92079034d4355ad3658/parachains/runtimes/assets/statemine/src/weights/xcm/mod.rs#L58-L61
//#[cfg(feature = "with-karura-runtime")]
//mod statemine;

#[cfg(feature = "with-acala-runtime")]
mod polkadot_cross_chain_transfer;
#[cfg(feature = "with-acala-runtime")]
pub mod polkadot_test_net;
// NOTE: Unsupport now
// https://github.com/paritytech/cumulus/blob/26e17fea7ef95d91b94db92079034d4355ad3658/parachains/runtimes/assets/statemint/src/weights/xcm/mod.rs#L58-L61
//#[cfg(feature = "with-acala-runtime")]
//mod statemint;

pub use fee_test::{relay_per_second_as_fee, token_per_second_as_fee};
use frame_support::weights::{constants::WEIGHT_PER_SECOND, Weight};
use sp_runtime::{FixedPointNumber, FixedU128};

// N * unit_weight * (weight/10^12) * token_per_second
fn weight_calculation(instruction_count: u32, unit_weight: Weight, per_second: u128) -> u128 {
	let weight = unit_weight.saturating_mul(instruction_count as u64);
	let weight_ratio = FixedU128::saturating_from_rational(weight.ref_time(), WEIGHT_PER_SECOND.ref_time());
	weight_ratio.saturating_mul_int(per_second)
}

// N * unit_weight * (weight/10^12) * token_per_second *
// (minimal_balance/native_asset_minimal_balance)
fn foreign_asset_fee(weight: u128, minimal_balance: u128) -> u128 {
	use crate::setup::Balances;
	use frame_support::traits::fungible::Inspect;

	let minimum_ratio = FixedU128::saturating_from_rational(minimal_balance, Balances::minimum_balance());
	minimum_ratio.saturating_mul_int(weight)
}

// N * unit_weight * (weight/10^12) * token_per_second * token_rate
fn token_asset_fee(weight: u128, token_rate: FixedU128) -> u128 {
	token_rate.saturating_mul_int(weight as u128)
}

mod fee_test {
	use super::{foreign_asset_fee, token_asset_fee, weight_calculation};
	use crate::setup::*;

	fn native_unit_cost(instruction_count: u32, per_second: u128) -> u128 {
		#[cfg(feature = "with-karura-runtime")]
		let unit_weight: Weight = Weight::from_ref_time(karura_runtime::xcm_config::UnitWeightCost::get());
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(unit_weight, Weight::from_ref_time(200_000_000));
		#[cfg(feature = "with-acala-runtime")]
		let unit_weight: Weight = Weight::from_ref_time(acala_runtime::xcm_config::UnitWeightCost::get());
		#[cfg(feature = "with-acala-runtime")]
		assert_eq!(unit_weight, Weight::from_ref_time(200_000_000));
		#[cfg(feature = "with-mandala-runtime")]
		let unit_weight: Weight = Weight::from_ref_time(mandala_runtime::xcm_config::UnitWeightCost::get());
		#[cfg(feature = "with-mandala-runtime")]
		assert_eq!(unit_weight, Weight::from_ref_time(1_000_000));

		weight_calculation(instruction_count, unit_weight, per_second)
	}

	pub fn relay_per_second_as_fee(instruction_count: u32) -> u128 {
		#[cfg(feature = "with-karura-runtime")]
		let relay_per_second = karura_runtime::ksm_per_second();
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(231_740_000_000, relay_per_second);

		#[cfg(feature = "with-acala-runtime")]
		let relay_per_second = acala_runtime::dot_per_second();
		#[cfg(feature = "with-acala-runtime")]
		assert_eq!(231_740_000_0, relay_per_second);

		#[cfg(feature = "with-mandala-runtime")]
		let relay_per_second = mandala_runtime::dot_per_second();
		#[cfg(feature = "with-mandala-runtime")]
		assert_eq!(115_870_000_000, relay_per_second);

		native_unit_cost(instruction_count, relay_per_second)
	}

	pub fn native_per_second_as_fee(instruction_count: u32) -> u128 {
		#[cfg(feature = "with-karura-runtime")]
		let native_per_second = karura_runtime::kar_per_second();
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(11_587_000_000_000, native_per_second);
		#[cfg(feature = "with-acala-runtime")]
		let native_per_second = acala_runtime::aca_per_second();
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(11_587_000_000_000, native_per_second);
		#[cfg(feature = "with-mandala-runtime")]
		let native_per_second = mandala_runtime::aca_per_second();
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(11_587_000_000_000, native_per_second);

		native_unit_cost(instruction_count, native_per_second)
	}

	#[cfg(feature = "with-karura-runtime")]
	pub fn bnc_per_second_as_fee(instruction_count: u32) -> u128 {
		relay_per_second_as_fee(instruction_count) * 80
	}

	pub fn foreign_per_second_as_fee(instruction_count: u32, minimal_balance: u128) -> u128 {
		#[cfg(feature = "with-karura-runtime")]
		let native_per_second = karura_runtime::kar_per_second();
		#[cfg(feature = "with-acala-runtime")]
		let native_per_second = acala_runtime::aca_per_second();
		#[cfg(feature = "with-mandala-runtime")]
		let native_per_second = mandala_runtime::aca_per_second();

		let weight = native_unit_cost(instruction_count, native_per_second);

		foreign_asset_fee(weight, minimal_balance)
	}

	pub fn token_per_second_as_fee(instruction_count: u32, rate: FixedU128) -> u128 {
		let native_fee = native_per_second_as_fee(instruction_count);
		token_asset_fee(native_fee, rate)
	}

	#[cfg(feature = "with-karura-runtime")]
	#[test]
	fn karura_per_second_works() {
		assert_eq!(185_392_000, relay_per_second_as_fee(4));
		assert_eq!(139_044_000, relay_per_second_as_fee(3));
		assert_eq!(9_269_600_000, native_per_second_as_fee(4));
		assert_eq!(14_831_360_000, bnc_per_second_as_fee(4));

		assert_eq!(9_269_600_000, foreign_per_second_as_fee(4, Balances::minimum_balance()));
		assert_eq!(
			926_960_000,
			foreign_per_second_as_fee(4, Balances::minimum_balance() / 10)
		);
	}

	#[cfg(feature = "with-acala-runtime")]
	#[test]
	fn acala_per_second_works() {
		assert_eq!(1_853_920, relay_per_second_as_fee(4));
		assert_eq!(1_390_440, relay_per_second_as_fee(3));
		assert_eq!(9_269_600_000, native_per_second_as_fee(4));

		assert_eq!(9_269_600_000, foreign_per_second_as_fee(4, Balances::minimum_balance()));
		assert_eq!(
			926_960_000,
			foreign_per_second_as_fee(4, Balances::minimum_balance() / 10)
		);
	}

	#[cfg(feature = "with-mandala-runtime")]
	#[test]
	fn mandala_per_second_works() {
		assert_eq!(463_480, relay_per_second_as_fee(4));
		assert_eq!(347_610, relay_per_second_as_fee(3));
		assert_eq!(46_348_000, native_per_second_as_fee(4));

		assert_eq!(46_348_000, foreign_per_second_as_fee(4, Balances::minimum_balance()));
		assert_eq!(
			4_634_800,
			foreign_per_second_as_fee(4, Balances::minimum_balance() / 10)
		);
	}
}

#[test]
fn weight_to_fee_works() {
	#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
	use frame_support::weights::{Weight, WeightToFee as WeightToFeeT};

	// Kusama
	#[cfg(feature = "with-karura-runtime")]
	{
		use kusama_runtime_constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(kusama_runtime::xcm_config::BaseXcmWeight::get());
		assert_eq!(base_weight, Weight::from_ref_time(1_000_000_000));

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(1_39_502_112, fee);

		// transfer_to_relay_chain weight in KusamaNet
		let weight: Weight = Weight::from_ref_time(298_368_000);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(10_405_742, fee);
	}

	// Polkadot
	#[cfg(feature = "with-acala-runtime")]
	{
		use polkadot_runtime_constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(polkadot_runtime::xcm_config::BaseXcmWeight::get());
		assert_eq!(base_weight, Weight::from_ref_time(1_000_000_000));

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(426_530_176, fee);

		// transfer_to_relay_chain weight in KusamaNet
		let weight: Weight = Weight::from_ref_time(298_368_000);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(31_815_739, fee);
	}

	// Statemine
	#[cfg(feature = "with-karura-runtime")]
	{
		use statemine_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(15_450_332, fee);
	}

	// Statemint
	#[cfg(feature = "with-acala-runtime")]
	{
		use statemint_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(46_351_012, fee);
	}

	// Karura
	#[cfg(feature = "with-karura-runtime")]
	{
		use karura_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(karura_runtime::xcm_config::BaseXcmWeight::get());
		assert_eq!(base_weight, Weight::from_ref_time(100_000_000));

		let unit_weight: Weight = Weight::from_ref_time(karura_runtime::xcm_config::UnitWeightCost::get());
		assert_eq!(unit_weight, Weight::from_ref_time(200_000_000));

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(4_635_101_624, fee);

		let weight: Weight = unit_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(9_270_203_249, fee);
	}

	// Acala
	#[cfg(feature = "with-acala-runtime")]
	{
		use acala_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(acala_runtime::xcm_config::BaseXcmWeight::get());
		assert_eq!(base_weight, Weight::from_ref_time(100_000_000));

		let unit_weight: Weight = Weight::from_ref_time(acala_runtime::xcm_config::UnitWeightCost::get());
		assert_eq!(unit_weight, Weight::from_ref_time(200_000_000));

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(4_635_101_624, fee);

		let weight: Weight = unit_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(9_270_203_249, fee);
	}
}
