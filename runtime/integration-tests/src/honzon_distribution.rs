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

//! Unit tests for Honzon distribution module.

#![cfg(test)]

use crate::evm::alice_evm_addr;
use crate::setup::*;
use crate::stable_asset::enable_3usd_pool;
use module_honzon_distribution::{DistributionDestination, DistributionToStableAsset};

fn first_mint() -> (DistributionDestination<AccountId>, Balance, AccountId) {
	let dollar = dollar(NATIVE_CURRENCY);
	let alith = MockAddressMapping::get_account_id(&alice_evm_addr());

	// aUSD proportion of total supply is 1/3.
	enable_3usd_pool(alith);

	// treasury account should have enough aUSD to mint stable asset pool.
	let treasury_account = TreasuryAccount::get();
	assert_ok!(Currencies::update_balance(
		Origin::root(),
		MultiAddress::Id(treasury_account.clone()),
		USD_CURRENCY,
		1_000_000 * dollar as i128,
	));

	let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
		pool_id: 0,
		stable_token_index: 2, // USD_CURRENCY
		account_id: treasury_account.clone(),
	};
	let destination = DistributionDestination::StableAsset(distribution_to_stable_asset);
	assert_ok!(HonzonDistribution::update_params(
		Origin::root(),
		destination.clone(),
		Some(1_000_000_000_000_000),
		Some(1_000_000_000_000_000),
		Some(Ratio::saturating_from_rational(38, 100)),
		Some(Ratio::saturating_from_rational(50, 100)),
	));
	assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
	let mint_amount = 225_806_451_612_903;
	System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::Minted {
		minter: treasury_account.clone(),
		pool_id: 0,
		a: 1000,
		input_amounts: vec![0, 0, 225_806_451_612_903],
		min_output_amount: 0,
		balances: vec![1000 * dollar, 1000 * dollar, 1000 * dollar + mint_amount],
		total_supply: 3_225_638_541_406_905,
		fee_amount: 225_638_541_406,
		output_amount: 225_412_902_865_499,
	}));
	System::assert_has_event(Event::HonzonDistribution(
		module_honzon_distribution::Event::AdjustDestination {
			destination: destination.clone(),
			amount: mint_amount as i128,
		},
	));
	let distributed = module_honzon_distribution::DistributedBalance::<Runtime>::get(&destination).unwrap();
	assert_eq!(distributed, mint_amount);
	assert_eq!(
		Tokens::free_balance(CurrencyId::StableAssetPoolToken(0), &treasury_account),
		225_412_902_865_499
	);

	(destination, mint_amount, treasury_account)
}

#[test]
fn remove_distribution_works() {
	let dollar = dollar(NATIVE_CURRENCY);
	let alith = MockAddressMapping::get_account_id(&alice_evm_addr());

	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000_000 * dollar),
			(alith.clone(), NATIVE_CURRENCY, 1_000_000_000 * dollar),
			(alith.clone(), USD_CURRENCY, 1_000_000_000 * dollar),
		])
		.build()
		.execute_with(|| {
			let (destination, _mint_amount, treasury_account) = first_mint();
			assert_eq!(
				Tokens::free_balance(CurrencyId::StableAssetPoolToken(0), &treasury_account),
				225_412_902_865_499
			);
			assert_ok!(HonzonDistribution::remove_distribution(
				Origin::root(),
				destination.clone()
			));
			assert_eq!(
				module_honzon_distribution::DistributedBalance::<Runtime>::get(&destination),
				None
			);
			assert_eq!(
				Tokens::free_balance(CurrencyId::StableAssetPoolToken(0), &treasury_account),
				0
			);
		});
}

#[test]
fn honzon_distribution_mint_burn_works() {
	let dollar = dollar(NATIVE_CURRENCY);
	let alith = MockAddressMapping::get_account_id(&alice_evm_addr());

	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000_000 * dollar),
			(alith.clone(), NATIVE_CURRENCY, 1_000_000_000 * dollar),
			(alith.clone(), USD_CURRENCY, 1_000_000_000 * dollar),
		])
		.build()
		.execute_with(|| {
			let (destination, mint_amount, treasury_account) = first_mint();

			let redeem_amount = 100_510_332_095_017;
			assert_ok!(HonzonDistribution::update_params(
				Origin::root(),
				destination.clone(),
				Some(1_000_000_000_000_000),
				Some(1_000_000_000_000_000),
				Some(Ratio::saturating_from_rational(30, 100)),
				Some(Ratio::saturating_from_rational(36, 100)),
			));
			assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
			System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
				redeemer: treasury_account.clone(),
				pool_id: 0,
				a: 1000,
				input_amount: 100_900_901_103_778,
				output_asset: USD_CURRENCY,
				min_output_amount: 0,
				balances: vec![1000 * dollar, 1000 * dollar, 1_125_296_119_517_886],
				total_supply: 3_125_242_144_808_645,
				fee_amount: 504_504_505_518,
				output_amount: redeem_amount,
			}));
			System::assert_has_event(Event::HonzonDistribution(
				module_honzon_distribution::Event::AdjustDestination {
					destination: destination.clone(),
					amount: 0_i128 - redeem_amount as i128,
				},
			));
			assert_eq!(
				module_honzon_distribution::DistributedBalance::<Runtime>::get(&destination).unwrap(),
				mint_amount - redeem_amount
			);
			let distributed = module_honzon_distribution::DistributedBalance::<Runtime>::get(&destination).unwrap();
			assert_eq!(distributed, 125_296_119_517_886);
			assert_eq!(
				Tokens::free_balance(CurrencyId::StableAssetPoolToken(0), &treasury_account),
				124_512_001_761_721
			);

			let redeem_amount = 123_943_409_764_532;
			let lp_share = 1_352_709_753_354;
			assert_ok!(HonzonDistribution::update_params(
				Origin::root(),
				destination.clone(),
				Some(0),
				Some(1_000_000_000_000_000),
				Some(Ratio::saturating_from_rational(30, 100)),
				Some(Ratio::saturating_from_rational(36, 100)),
			));
			assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
			System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
				redeemer: treasury_account.clone(),
				pool_id: 0,
				a: 1000,
				input_amount: 124_512_001_761_721,
				output_asset: USD_CURRENCY,
				min_output_amount: 0,
				balances: vec![1000 * dollar, 1000 * dollar, 1000 * dollar + lp_share],
				total_supply: 3001352703055733,
				fee_amount: 622560008808,
				output_amount: redeem_amount,
			}));
			System::assert_has_event(Event::HonzonDistribution(
				module_honzon_distribution::Event::AdjustDestination {
					destination: destination.clone(),
					amount: 0_i128 - redeem_amount as i128,
				},
			));
			assert_eq!(
				module_honzon_distribution::DistributedBalance::<Runtime>::get(&destination).unwrap(),
				lp_share
			);
			assert_eq!(
				Tokens::free_balance(CurrencyId::StableAssetPoolToken(0), &treasury_account),
				0
			);
		});
}
