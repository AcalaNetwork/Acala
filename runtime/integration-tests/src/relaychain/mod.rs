// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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
#[cfg(feature = "with-karura-runtime")]
mod statemine;

#[cfg(feature = "with-acala-runtime")]
mod polkadot_cross_chain_transfer;
#[cfg(feature = "with-acala-runtime")]
pub mod polkadot_test_net;
#[cfg(feature = "with-acala-runtime")]
mod statemint;

pub use fee_test::{relay_per_second_as_fee, token_per_second_as_fee};
use frame_support::weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight};
use sp_runtime::{FixedPointNumber, FixedU128};

// N * unit_weight * (weight/10^12) * token_per_second
fn weight_calculation(instruction_count: u32, unit_weight: Weight, per_second: u128) -> u128 {
	let weight = unit_weight.saturating_mul(instruction_count as u64);
	let weight_ratio = FixedU128::saturating_from_rational(weight.ref_time(), WEIGHT_REF_TIME_PER_SECOND);
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
		assert_eq!(202_060_000_000, relay_per_second);

		#[cfg(feature = "with-acala-runtime")]
		let relay_per_second = acala_runtime::dot_per_second();
		#[cfg(feature = "with-acala-runtime")]
		assert_eq!(2_020_600_000, relay_per_second);

		#[cfg(feature = "with-mandala-runtime")]
		let relay_per_second = mandala_runtime::dot_per_second();
		#[cfg(feature = "with-mandala-runtime")]
		assert_eq!(101_030_000_000, relay_per_second);

		native_unit_cost(instruction_count, relay_per_second)
	}

	pub fn native_per_second_as_fee(instruction_count: u32) -> u128 {
		#[cfg(feature = "with-karura-runtime")]
		let native_per_second = karura_runtime::kar_per_second();
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(10_103_000_000_000, native_per_second);
		#[cfg(feature = "with-acala-runtime")]
		let native_per_second = acala_runtime::aca_per_second();
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(10_103_000_000_000, native_per_second);
		#[cfg(feature = "with-mandala-runtime")]
		let native_per_second = mandala_runtime::aca_per_second();
		#[cfg(feature = "with-karura-runtime")]
		assert_eq!(10_103_000_000_000, native_per_second);

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
		assert_eq!(161_648_000, relay_per_second_as_fee(4));
		assert_eq!(121_236_000, relay_per_second_as_fee(3));
		assert_eq!(8_082_400_000, native_per_second_as_fee(4));
		assert_eq!(12_931_840_000, bnc_per_second_as_fee(4));

		assert_eq!(8_082_400_000, foreign_per_second_as_fee(4, Balances::minimum_balance()));
		assert_eq!(
			808_240_000,
			foreign_per_second_as_fee(4, Balances::minimum_balance() / 10)
		);
	}

	#[cfg(feature = "with-acala-runtime")]
	#[test]
	fn acala_per_second_works() {
		assert_eq!(1_616_480, relay_per_second_as_fee(4));
		assert_eq!(1_212_360, relay_per_second_as_fee(3));
		assert_eq!(8_082_400_000, native_per_second_as_fee(4));

		assert_eq!(8_082_400_000, foreign_per_second_as_fee(4, Balances::minimum_balance()));
		assert_eq!(
			808_240_000,
			foreign_per_second_as_fee(4, Balances::minimum_balance() / 10)
		);
	}

	#[cfg(feature = "with-mandala-runtime")]
	#[test]
	fn mandala_per_second_works() {
		assert_eq!(404_120, relay_per_second_as_fee(4));
		assert_eq!(303_090, relay_per_second_as_fee(3));
		assert_eq!(40_412_000, native_per_second_as_fee(4));

		assert_eq!(40_412_000, foreign_per_second_as_fee(4, Balances::minimum_balance()));
		assert_eq!(
			4_041_200,
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
		assert_eq!(1_401_915_012, fee);

		// transfer_to_relay_chain weight in KusamaNet
		let weight: Weight = Weight::from_ref_time(298_368_000);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(104_571_645, fee);
	}

	// Polkadot
	#[cfg(feature = "with-acala-runtime")]
	{
		use polkadot_runtime_constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(polkadot_runtime::xcm_config::BaseXcmWeight::get());
		assert_eq!(base_weight, Weight::from_ref_time(1_000_000_000));

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(421_434_140, fee);

		// transfer_to_relay_chain weight in KusamaNet
		let weight: Weight = Weight::from_ref_time(298_368_000);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(31_435_615, fee);
	}

	// Statemine
	#[cfg(feature = "with-karura-runtime")]
	{
		use statemine_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(134_715_512, fee);
	}

	// Statemint
	#[cfg(feature = "with-acala-runtime")]
	{
		use statemint_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = Weight::from_ref_time(1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(40_414_652, fee);
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
		assert_eq!(4_041_465_435, fee);

		let weight: Weight = unit_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(8_082_930_870, fee);
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
		assert_eq!(4_041_465_435, fee);

		let weight: Weight = unit_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(8_082_930_870, fee);
	}
}
