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

use super::*;
use crate::mock::{Event, *};
use frame_support::{assert_noop, assert_ok};
use nutsfinance_stable_asset::traits::StableAsset as StableAssetT;

fn inject_liquidity(
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> DispatchResult {
	// set balance
	Tokens::deposit(currency_id_a, &ALICE, max_amount_a)?;
	Tokens::deposit(currency_id_b, &ALICE, max_amount_b)?;

	let _ = Dex::enable_trading_pair(Origin::root(), currency_id_a, currency_id_b);
	Dex::add_liquidity(
		Origin::signed(ALICE),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		false,
	)?;

	Ok(())
}

fn initial_stable_asset(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> DispatchResult {
	let amount = 100_000_000_000_000u128;
	StableAssetWrapper::create_pool(
		STABLE_ASSET,
		vec![currency_id_a, currency_id_b],
		vec![1u128, 1u128],
		0,
		0,
		0,
		3000u128,
		BOB,
		BOB,
		10_000_000_000u128,
	)?;

	Tokens::deposit(currency_id_a, &BOB, amount)?;
	Tokens::deposit(currency_id_b, &BOB, amount)?;

	StableAssetWrapper::mint(&BOB, 0, vec![amount, amount], 0)?;
	assert_eq!(
		StableAssetWrapper::pool(0).map(|p| p.balances).unwrap(),
		vec![amount, amount]
	);

	Ok(())
}

#[test]
fn update_params_works() {
	ExtBuilder::default().build().execute_with(|| {
		let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
			pool_id: 0,
			stable_token_index: 0,
			stable_currency_id: AUSD,
			account_id: CHARLIE,
		};
		let destination = DistributionDestination::StableAsset(distribution_to_stable_asset);

		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1000),
			None,
			None,
			None
		));

		System::assert_last_event(Event::HonzonDistribution(crate::Event::UpdateDistributionParams {
			destination,
			params: DistributionParams {
				capacity: 1000,
				max_step: 0,
				target_min: Default::default(),
				target_max: Default::default(),
			},
		}));
	});
}

#[test]
fn stable_asset_mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = inject_liquidity(ACA, AUSD, 100_000_000_000_000, 200_000_000_000_000);
		let _ = inject_liquidity(AUSD, DOT, 100_000_000_000_000, 200_000_000_000_000);

		let _ = initial_stable_asset(DOT, LDOT);
		let _ = Tokens::deposit(DOT, &ALICE, 100_000_000_000u128);
		let swap_output = StableAssetWrapper::get_swap_output_amount(0, 0, 1, 1_000_000_000u128).unwrap();
		let (input, output) = StableAssetWrapper::swap(&ALICE, 0, 0, 1, 1_000_000_000u128, 0, 2).unwrap();
		assert_eq!(swap_output.dx, input);
		assert_eq!(swap_output.dy, output);

		let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
			pool_id: 0,
			stable_token_index: 0,
			stable_currency_id: DOT,
			account_id: CHARLIE,
		};
		let destination = DistributionDestination::StableAsset(distribution_to_stable_asset);

		// Without `DistributedBalance`, current mint amount exceed capacity
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(70, 100)),
			Some(Ratio::saturating_from_rational(80, 100)),
		));
		assert_noop!(
			HonzonDistribution::force_adjust(Origin::root(), destination.clone()),
			Error::<Runtime>::ExceedCapacity
		);

		// normal capacity
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(6, 10)),
			Some(Ratio::saturating_from_rational(7, 10)),
		));

		let _ = Tokens::deposit(DOT, &CHARLIE, 100_000_000_000_000);
		let _ = Tokens::deposit(LDOT, &CHARLIE, 100_000_000_000_000);
		StableAssetWrapper::mint(&CHARLIE, 0, vec![100_000_000_000_000, 100_000_000_000_000], 0).unwrap();

		// less than target, mint aUSD
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		let ausd_mint = 171_425_714_285_865u128;
		let stable_mint = 370_963_593_384_271u128;
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::Minted {
			minter: CHARLIE,
			pool_id: 0,
			a: 3000,
			input_amounts: vec![ausd_mint, 0],
			min_output_amount: 0,
			balances: vec![371_426_714_285_865, 199_999_000_000_165],
			total_supply: 570_963_593_384_272,
			fee_amount: 0,
			output_amount: 170_963_593_384_189,
		}));
		// minted aUSD is add to `DistributedBalance`, and lp go to minter.
		assert_eq!(DistributedBalance::<Runtime>::get(&destination).unwrap(), ausd_mint);
		assert_eq!(Tokens::free_balance(STABLE_ASSET, &CHARLIE), stable_mint);
		// latest target = 0.6505

		// larger than target, burn aUSD
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(60, 100)),
			Some(Ratio::saturating_from_rational(65, 100)),
		));
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		let stable_redeem = 38_865_249_121_853u128;
		let ausd_burn = 39_040_204_684_665u128;
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
			redeemer: CHARLIE,
			pool_id: 0,
			a: 3000,
			input_amount: stable_redeem,
			output_asset: DOT,
			min_output_amount: 0,
			balances: vec![332_386_509_601_200, 199_999_000_000_165],
			total_supply: 532_098_344_262_419,
			fee_amount: 0,
			output_amount: ausd_burn,
		}));
		// redeemed aUSD is reduce from `DistributedBalance`, and lp also reduce from minter.
		assert_eq!(
			DistributedBalance::<Runtime>::get(&destination).unwrap(),
			ausd_mint - ausd_burn
		);
		assert_eq!(
			Tokens::free_balance(STABLE_ASSET, &CHARLIE),
			stable_mint - stable_redeem
		);
		// latest target = 0.6246

		// existing `DistributedBalance` add mint amount exceed capacity
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(70, 100)),
			Some(Ratio::saturating_from_rational(80, 100)),
		));
		assert_noop!(
			HonzonDistribution::force_adjust(Origin::root(), destination.clone()),
			Error::<Runtime>::ExceedCapacity
		);

		// burned amount is large than `DistributedBalance`, failed.
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(48, 100)),
			Some(Ratio::saturating_from_rational(52, 100)),
		));
		System::reset_events();
		assert_noop!(
			HonzonDistribution::force_adjust(Origin::root(), destination.clone()),
			Error::<Runtime>::InvalidUpdateBalance
		);
		assert_eq!(
			System::events().iter().find(|r| matches!(
				r.event,
				Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle { .. })
			)),
			None
		);

		assert_eq!(
			DistributedBalance::<Runtime>::get(&destination).unwrap(),
			ausd_mint - ausd_burn
		);
		assert_eq!(
			Tokens::free_balance(STABLE_ASSET, &CHARLIE),
			stable_mint - stable_redeem
		);

		// burned amount is less than `DistributedBalance`, success.
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(49, 100)),
			Some(Ratio::saturating_from_rational(52, 100)),
		));
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		assert!(System::events().iter().any(|r| {
			matches!(
				r.event,
				Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle { .. })
			)
		}));
	});
}
