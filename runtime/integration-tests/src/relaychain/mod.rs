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

#[cfg(feature = "with-karura-runtime")]
mod kusama_cross_chain_transfer;
#[cfg(feature = "with-karura-runtime")]
pub mod kusama_test_net;
#[cfg(feature = "with-acala-runtime")]
mod polkadot_cross_chain_transfer;
#[cfg(feature = "with-acala-runtime")]
pub mod polkadot_test_net;
mod relay_chain;
#[cfg(feature = "with-karura-runtime")]
mod statemine;
#[cfg(feature = "with-acala-runtime")]
mod statemint;

#[cfg(feature = "with-karura-runtime")]
mod erc20;

#[test]
fn weight_to_fee_works() {
	use frame_support::weights::{Weight, WeightToFee as WeightToFeeT};

	// Kusama
	#[cfg(feature = "with-karura-runtime")]
	{
		use kusama_runtime_constants::fee::WeightToFee;

		let base_weight: Weight = kusama_runtime::xcm_config::BaseXcmWeight::get();
		assert_eq!(base_weight, 1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(154_483_692, fee);

		// transfer_to_relay_chain weight in KusamaNet
		let weight: Weight = 298_368_000;
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(11_523_248, fee);
	}

	// Polkadot
	#[cfg(feature = "with-acala-runtime")]
	{
		use polkadot_runtime_constants::fee::WeightToFee;

		let base_weight: Weight = polkadot_runtime::xcm_config::BaseXcmWeight::get();
		assert_eq!(base_weight, 1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(469_417_452, fee);

		// transfer_to_relay_chain weight in KusamaNet
		let weight: Weight = 298_368_000;
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(35_014_787, fee);
	}

	// Statemine
	#[cfg(feature = "with-karura-runtime")]
	{
		use statemine_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = statemine_runtime::xcm_config::UnitWeightCost::get();
		assert_eq!(base_weight, 1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(15_540_916, fee);
	}

	// Statemint
	#[cfg(feature = "with-acala-runtime")]
	{
		use statemint_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = statemint_runtime::xcm_config::UnitWeightCost::get();
		assert_eq!(base_weight, 1_000_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(46_622_760, fee);
	}

	// Karura
	#[cfg(feature = "with-karura-runtime")]
	{
		use karura_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = karura_runtime::xcm_config::BaseXcmWeight::get();
		assert_eq!(base_weight, 100_000_000);

		let unit_weight: Weight = karura_runtime::xcm_config::UnitWeightCost::get();
		assert_eq!(unit_weight, 200_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(4_662_276_356, fee);

		let weight: Weight = unit_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(9_324_552_713, fee);
	}

	// Acala
	#[cfg(feature = "with-acala-runtime")]
	{
		use acala_runtime::constants::fee::WeightToFee;

		let base_weight: Weight = acala_runtime::xcm_config::BaseXcmWeight::get();
		assert_eq!(base_weight, 100_000_000);

		let unit_weight: Weight = acala_runtime::xcm_config::UnitWeightCost::get();
		assert_eq!(unit_weight, 200_000_000);

		let weight: Weight = base_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(4_662_276_356, fee);

		let weight: Weight = unit_weight.saturating_mul(4);
		let fee = WeightToFee::weight_to_fee(&weight);
		assert_eq!(9_324_552_713, fee);
	}
}
