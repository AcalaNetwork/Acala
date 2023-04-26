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

//! Common xcm implementation

use codec::Encode;
use frame_support::{traits::Get, weights::constants::WEIGHT_REF_TIME_PER_SECOND};
use module_support::BuyWeightRate;
use orml_traits::GetByKey;
use primitives::{evm::EvmAddress, Balance, CurrencyId};
use sp_core::bounded::BoundedVec;
use sp_runtime::{traits::Convert, FixedPointNumber, FixedU128};
use sp_std::{marker::PhantomData, prelude::*};
use xcm::{prelude::*, v3::Weight as XcmWeight};
use xcm_builder::TakeRevenue;
use xcm_executor::{
	traits::{DropAssets, WeightTrader},
	Assets,
};

pub fn local_currency_location(key: CurrencyId) -> Option<MultiLocation> {
	Some(MultiLocation::new(
		0,
		X1(Junction::from(BoundedVec::try_from(key.encode()).ok()?)),
	))
}

pub fn native_currency_location(para_id: u32, key: Vec<u8>) -> Option<MultiLocation> {
	Some(MultiLocation::new(
		1,
		X2(Parachain(para_id), Junction::from(BoundedVec::try_from(key).ok()?)),
	))
}

/// `ExistentialDeposit` for tokens, give priority to match native token, then handled by
/// `ExistentialDeposits`.
///
/// parameters type:
/// - `NC`: native currency_id type.
/// - `NB`: the ExistentialDeposit amount of native currency_id.
/// - `GK`: the ExistentialDeposit amount of tokens.
pub struct ExistentialDepositsForDropAssets<NC, NB, GK>(PhantomData<(NC, NB, GK)>);
impl<NC, NB, GK> ExistentialDepositsForDropAssets<NC, NB, GK>
where
	NC: Get<CurrencyId>,
	NB: Get<Balance>,
	GK: GetByKey<CurrencyId, Balance>,
{
	fn get(currency_id: &CurrencyId) -> Balance {
		if currency_id == &NC::get() {
			NB::get()
		} else {
			GK::get(currency_id)
		}
	}
}

/// `DropAssets` implementation support asset amount lower thant ED handled by `TakeRevenue`.
///
/// parameters type:
/// - `NC`: native currency_id type.
/// - `NB`: the ExistentialDeposit amount of native currency_id.
/// - `GK`: the ExistentialDeposit amount of tokens.
pub struct AcalaDropAssets<X, T, C, NC, NB, GK>(PhantomData<(X, T, C, NC, NB, GK)>);
impl<X, T, C, NC, NB, GK> DropAssets for AcalaDropAssets<X, T, C, NC, NB, GK>
where
	X: DropAssets,
	T: TakeRevenue,
	C: Convert<MultiLocation, Option<CurrencyId>>,
	NC: Get<CurrencyId>,
	NB: Get<Balance>,
	GK: GetByKey<CurrencyId, Balance>,
{
	fn drop_assets(origin: &MultiLocation, assets: Assets, context: &XcmContext) -> XcmWeight {
		let multi_assets: Vec<MultiAsset> = assets.into();
		let mut asset_traps: Vec<MultiAsset> = vec![];
		for asset in multi_assets {
			if let MultiAsset {
				id: Concrete(location),
				fun: Fungible(amount),
			} = asset.clone()
			{
				let currency_id = C::convert(location);
				// burn asset(do nothing here) if convert result is None
				if let Some(currency_id) = currency_id {
					let ed = ExistentialDepositsForDropAssets::<NC, NB, GK>::get(&currency_id);
					if amount < ed {
						T::take_revenue(asset);
					} else {
						asset_traps.push(asset);
					}
				}
			}
		}
		if !asset_traps.is_empty() {
			X::drop_assets(origin, asset_traps.into(), context);
		}
		// TODO #2492: Put the real weight in there.
		XcmWeight::from_ref_time(0)
	}
}

/// Simple fee calculator that requires payment in a single fungible at a fixed rate.
///
/// - The `FixedRate` constant should be the concrete fungible ID and the amount of it
/// required for one second of weight.
/// - The `TakeRevenue` trait is used to collecting xcm execution fee.
/// - The `BuyWeightRate` trait is used to calculate ratio by location.
pub struct FixedRateOfAsset<FixedRate: Get<u128>, R: TakeRevenue, M: BuyWeightRate> {
	weight: XcmWeight,
	amount: u128,
	ratio: FixedU128,
	multi_location: Option<MultiLocation>,
	_marker: PhantomData<(FixedRate, R, M)>,
}

impl<FixedRate: Get<u128>, R: TakeRevenue, M: BuyWeightRate> WeightTrader for FixedRateOfAsset<FixedRate, R, M> {
	fn new() -> Self {
		Self {
			weight: XcmWeight::zero(),
			amount: 0,
			ratio: Default::default(),
			multi_location: None,
			_marker: PhantomData,
		}
	}

	fn buy_weight(&mut self, weight: XcmWeight, payment: Assets) -> Result<Assets, XcmError> {
		log::trace!(target: "xcm::weight", "buy_weight weight: {:?}, payment: {:?}", weight, payment);

		// only support first fungible assets now.
		let asset_id = payment
			.fungible
			.iter()
			.next()
			.map_or(Err(XcmError::TooExpensive), |v| Ok(v.0))?;

		if let AssetId::Concrete(ref multi_location) = asset_id {
			log::debug!(target: "xcm::weight", "buy_weight multi_location: {:?}", multi_location);

			if let Some(ratio) = M::calculate_rate(*multi_location) {
				// The WEIGHT_REF_TIME_PER_SECOND is non-zero.
				let weight_ratio =
					FixedU128::saturating_from_rational(weight.ref_time() as u128, WEIGHT_REF_TIME_PER_SECOND as u128);
				let amount = ratio.saturating_mul_int(weight_ratio.saturating_mul_int(FixedRate::get()));

				let required = MultiAsset {
					id: *asset_id,
					fun: Fungible(amount),
				};

				log::trace!(
					target: "xcm::weight", "buy_weight payment: {:?}, required: {:?}, fixed_rate: {:?}, ratio: {:?}, weight_ratio: {:?}",
					payment, required, FixedRate::get(), ratio, weight_ratio
				);
				let unused = payment
					.clone()
					.checked_sub(required)
					.map_err(|_| XcmError::TooExpensive)?;
				self.weight = self.weight.saturating_add(weight);
				self.amount = self.amount.saturating_add(amount);
				self.ratio = ratio;
				self.multi_location = Some(*multi_location);
				return Ok(unused);
			}
		}

		log::trace!(target: "xcm::weight", "no concrete fungible asset");
		Err(XcmError::TooExpensive)
	}

	fn refund_weight(&mut self, weight: XcmWeight) -> Option<MultiAsset> {
		log::trace!(
			target: "xcm::weight", "refund_weight weight: {:?}, weight: {:?}, amount: {:?}, ratio: {:?}, multi_location: {:?}",
			weight, self.weight, self.amount, self.ratio, self.multi_location
		);
		let weight = weight.min(self.weight);
		let weight_ratio =
			FixedU128::saturating_from_rational(weight.ref_time() as u128, WEIGHT_REF_TIME_PER_SECOND as u128);
		let amount = self
			.ratio
			.saturating_mul_int(weight_ratio.saturating_mul_int(FixedRate::get()));

		self.weight = self.weight.saturating_sub(weight);
		self.amount = self.amount.saturating_sub(amount);

		log::trace!(target: "xcm::weight", "refund_weight amount: {:?}", amount);
		if amount > 0 && self.multi_location.is_some() {
			Some(
				(
					*self.multi_location.as_ref().expect("checked is non-empty; qed"),
					amount,
				)
					.into(),
			)
		} else {
			None
		}
	}
}

impl<FixedRate: Get<u128>, R: TakeRevenue, M: BuyWeightRate> Drop for FixedRateOfAsset<FixedRate, R, M> {
	fn drop(&mut self) {
		log::trace!(target: "xcm::weight", "take revenue, weight: {:?}, amount: {:?}, multi_location: {:?}", self.weight, self.amount, self.multi_location);
		if self.amount > 0 && self.multi_location.is_some() {
			R::take_revenue(
				(
					*self.multi_location.as_ref().expect("checked is non-empty; qed"),
					self.amount,
				)
					.into(),
			);
		}
	}
}

pub struct XcmExecutor<Config: xcm_executor::Config, AccountId, Balance, AccountIdConvert, EVMBridge>(
	PhantomData<(Config, AccountId, Balance, AccountIdConvert, EVMBridge)>,
);

impl<
		Config: xcm_executor::Config,
		AccountId: Clone,
		Balance,
		AccountIdConvert: xcm_executor::traits::Convert<MultiLocation, AccountId>,
		EVMBridge: module_support::EVMBridge<AccountId, Balance>,
	> ExecuteXcm<Config::RuntimeCall> for XcmExecutor<Config, AccountId, Balance, AccountIdConvert, EVMBridge>
{
	type Prepared = <xcm_executor::XcmExecutor<Config> as ExecuteXcm<Config::RuntimeCall>>::Prepared;

	fn prepare(message: Xcm<Config::RuntimeCall>) -> Result<Self::Prepared, Xcm<Config::RuntimeCall>> {
		xcm_executor::XcmExecutor::<Config>::prepare(message)
	}

	fn execute(
		origin: impl Into<MultiLocation>,
		weighed_message: Self::Prepared,
		message_hash: XcmHash,
		weight_credit: XcmWeight,
	) -> Outcome {
		let origin = origin.into();
		let account = AccountIdConvert::convert(origin);
		let clear = if let Ok(account) = account {
			EVMBridge::push_xcm_origin(account);
			true
		} else {
			false
		};

		let res = xcm_executor::XcmExecutor::<Config>::execute(origin, weighed_message, message_hash, weight_credit);

		if clear {
			EVMBridge::pop_xcm_origin();
		}
		res
	}

	fn charge_fees(origin: impl Into<MultiLocation>, fees: MultiAssets) -> XcmResult {
		xcm_executor::XcmExecutor::<Config>::charge_fees(origin, fees)
	}
}

/// Convert `AccountKey20` to `AccountId`
pub struct AccountKey20Aliases<Network, AccountId, AddressMapping>(PhantomData<(Network, AccountId, AddressMapping)>);
impl<Network, AccountId, AddressMapping> xcm_executor::traits::Convert<MultiLocation, AccountId>
	for AccountKey20Aliases<Network, AccountId, AddressMapping>
where
	Network: Get<Option<NetworkId>>,
	AccountId: From<[u8; 32]> + Into<[u8; 32]> + Clone,
	AddressMapping: module_support::AddressMapping<AccountId>,
{
	fn convert(location: MultiLocation) -> Result<AccountId, MultiLocation> {
		let key = match location {
			MultiLocation {
				parents: 0,
				interior: X1(AccountKey20 { key, network: None }),
			} => key,
			MultiLocation {
				parents: 0,
				interior: X1(AccountKey20 { key, network }),
			} if network == Network::get() => key,
			_ => return Err(location),
		};

		Ok(AddressMapping::get_account_id(&EvmAddress::from(key)))
	}

	fn reverse(who: AccountId) -> Result<MultiLocation, AccountId> {
		// NOTE: Not sure whether to use AccountId32 or AccountKey20, not implemented for now
		// Ok(AccountKey20 { key: AddressMapping::get_or_create_evm_address(who), network: Network::get()
		// }.into())
		// Ok(AccountId32 { id: who.into(), network: Network::get() }.into())
		Err(who)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::new_test_ext;
	use frame_support::{assert_noop, assert_ok, parameter_types};
	use module_support::Ratio;
	use sp_runtime::traits::One;

	pub struct MockNoneBuyWeightRate;
	impl BuyWeightRate for MockNoneBuyWeightRate {
		fn calculate_rate(_: MultiLocation) -> Option<Ratio> {
			None
		}
	}

	pub struct MockFixedBuyWeightRate<T: Get<Ratio>>(PhantomData<T>);
	impl<T: Get<Ratio>> BuyWeightRate for MockFixedBuyWeightRate<T> {
		fn calculate_rate(_: MultiLocation) -> Option<Ratio> {
			Some(T::get())
		}
	}

	parameter_types! {
		const FixedBasedRate: u128 = 10;
		FixedRate: Ratio = Ratio::one();
	}

	#[test]
	fn currency_id_encode_as_general_key_works() {
		use primitives::DexShare;
		use primitives::TokenSymbol::ACA;
		let evm_addr = sp_core::H160(hex_literal::hex!("0000000000000000000000000000000000000400"));

		assert_eq!(
			native_currency_location(0, CurrencyId::Token(ACA).encode())
				.unwrap()
				.parents,
			1
		);
		assert_eq!(
			native_currency_location(0, CurrencyId::Erc20(evm_addr).encode())
				.unwrap()
				.parents,
			1
		);
		assert_eq!(
			native_currency_location(0, CurrencyId::StableAssetPoolToken(0).encode())
				.unwrap()
				.parents,
			1
		);
		assert_eq!(
			native_currency_location(0, CurrencyId::ForeignAsset(0).encode())
				.unwrap()
				.parents,
			1
		);
		assert_eq!(
			native_currency_location(0, CurrencyId::LiquidCrowdloan(0).encode())
				.unwrap()
				.parents,
			1
		);

		assert_eq!(
			native_currency_location(
				0,
				CurrencyId::DexShare(DexShare::Token(ACA), DexShare::ForeignAsset(0)).encode()
			)
			.unwrap()
			.parents,
			1
		);
		assert_eq!(
			native_currency_location(
				0,
				CurrencyId::DexShare(DexShare::Token(ACA), DexShare::Erc20(evm_addr)).encode()
			)
			.unwrap()
			.parents,
			1
		);

		// DexShare of two Erc20 exceed 32 length
		assert_eq!(
			native_currency_location(
				0,
				CurrencyId::DexShare(DexShare::Erc20(evm_addr), DexShare::Erc20(evm_addr)).encode()
			),
			None
		);
	}

	#[test]
	fn buy_weight_rate_mock_works() {
		new_test_ext().execute_with(|| {
			let asset: MultiAsset = (Parent, 100).into();
			let assets: Assets = asset.into();
			let mut trader = <FixedRateOfAsset<(), (), MockNoneBuyWeightRate>>::new();
			let buy_weight = trader.buy_weight(XcmWeight::from_ref_time(WEIGHT_REF_TIME_PER_SECOND), assets.clone());
			assert_noop!(buy_weight, XcmError::TooExpensive);

			let mut trader = <FixedRateOfAsset<FixedBasedRate, (), MockFixedBuyWeightRate<FixedRate>>>::new();
			let buy_weight = trader.buy_weight(XcmWeight::from_ref_time(WEIGHT_REF_TIME_PER_SECOND), assets.clone());
			let asset: MultiAsset = (Parent, 90).into();
			let assets: Assets = asset.into();
			assert_ok!(buy_weight, assets.clone());
		});
	}
}
