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

//! Common xcm implementation

use frame_support::{
	traits::Get,
	weights::{constants::WEIGHT_PER_SECOND, Weight},
};
use module_support::BuyWeightRate;
use sp_runtime::{FixedPointNumber, FixedU128};
use sp_std::marker::PhantomData;
use xcm::latest::prelude::*;
use xcm_builder::TakeRevenue;
use xcm_executor::{traits::WeightTrader, Assets};

/// Simple fee calculator that requires payment in a single fungible at a fixed rate.
///
/// The constant `FixedRate` type parameter should be the concrete fungible ID and the amount of it
/// required for one second of weight.
pub struct FixedRateOfAssetRegistry<FixedRate: Get<u128>, R: TakeRevenue, M: BuyWeightRate> {
	weight: Weight,
	amount: u128,
	ratio: FixedU128,
	multi_location: Option<MultiLocation>,
	_marker: PhantomData<(FixedRate, R, M)>,
}

impl<FixedRate: Get<u128>, R: TakeRevenue, M: BuyWeightRate> WeightTrader
	for FixedRateOfAssetRegistry<FixedRate, R, M>
{
	fn new() -> Self {
		Self {
			weight: 0,
			amount: 0,
			ratio: Default::default(),
			multi_location: None,
			_marker: PhantomData,
		}
	}

	fn buy_weight(&mut self, weight: Weight, payment: Assets) -> Result<Assets, XcmError> {
		log::trace!(target: "asset-registry::weight", "buy_weight weight: {:?}, payment: {:?}", weight, payment);

		// only support first fungible assets now.
		let asset_id = payment
			.fungible
			.iter()
			.next()
			.map_or(Err(XcmError::TooExpensive), |v| Ok(v.0))?;

		if let AssetId::Concrete(ref multi_location) = asset_id {
			log::debug!(target: "asset-registry::weight", "buy_weight multi_location: {:?}", multi_location);

			if let Some(ratio) = M::calculate_rate(multi_location.clone()) {
				// The WEIGHT_PER_SECOND is non-zero.
				let weight_ratio = FixedU128::saturating_from_rational(weight as u128, WEIGHT_PER_SECOND as u128);
				let amount = ratio.saturating_mul_int(weight_ratio.saturating_mul_int(FixedRate::get()));

				let required = MultiAsset {
					id: asset_id.clone(),
					fun: Fungible(amount),
				};

				log::trace!(
					target: "asset-registry::weight", "buy_weight payment: {:?}, required: {:?}, fixed_rate: {:?}, ratio: {:?}, weight_ratio: {:?}",
					payment, required, FixedRate::get(), ratio, weight_ratio
				);
				let unused = payment
					.clone()
					.checked_sub(required)
					.map_err(|_| XcmError::TooExpensive)?;
				self.weight = self.weight.saturating_add(weight);
				self.amount = self.amount.saturating_add(amount);
				self.ratio = ratio;
				self.multi_location = Some(multi_location.clone());
				return Ok(unused);
			}
		}

		log::trace!(target: "asset-registry::weight", "no concrete fungible asset");
		Err(XcmError::TooExpensive)
	}

	fn refund_weight(&mut self, weight: Weight) -> Option<MultiAsset> {
		log::trace!(
			target: "asset-registry::weight", "refund_weight weight: {:?}, weight: {:?}, amount: {:?}, ratio: {:?}, multi_location: {:?}",
			weight, self.weight, self.amount, self.ratio, self.multi_location
		);
		let weight = weight.min(self.weight);
		let weight_ratio = FixedU128::saturating_from_rational(weight as u128, WEIGHT_PER_SECOND as u128);
		let amount = self
			.ratio
			.saturating_mul_int(weight_ratio.saturating_mul_int(FixedRate::get()));

		self.weight = self.weight.saturating_sub(weight);
		self.amount = self.amount.saturating_sub(amount);

		log::trace!(target: "asset-registry::weight", "refund_weight amount: {:?}", amount);
		if amount > 0 && self.multi_location.is_some() {
			Some(
				(
					self.multi_location.as_ref().expect("checked is non-empty; qed").clone(),
					amount,
				)
					.into(),
			)
		} else {
			None
		}
	}
}

impl<FixedRate: Get<u128>, R: TakeRevenue, M: BuyWeightRate> Drop for FixedRateOfAssetRegistry<FixedRate, R, M> {
	fn drop(&mut self) {
		log::trace!(target: "asset-registry::weight", "take revenue, weight: {:?}, amount: {:?}, multi_location: {:?}", self.weight, self.amount, self.multi_location);
		if self.amount > 0 && self.multi_location.is_some() {
			R::take_revenue(
				(
					self.multi_location.as_ref().expect("checked is non-empty; qed").clone(),
					self.amount,
				)
					.into(),
			);
		}
	}
}
