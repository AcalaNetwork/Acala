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

		// stable asset params is not correct when doing real adjust.
		assert_ok!(initial_stable_asset(AUSD, LDOT));
		let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
			pool_id: 0,
			stable_token_index: 2,
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
		assert_noop!(
			HonzonDistribution::force_adjust(Origin::root(), destination.clone()),
			Error::<Runtime>::InvalidDestination
		);
	});
}

fn update_params(destination: DistributionDestination<AccountId>, lower: u32, upper: u32) {
	let lower_base = if lower > 100 { 1000 } else { 100 };
	let upper_base = if upper > 100 { 1000 } else { 100 };
	assert_ok!(HonzonDistribution::update_params(
		Origin::root(),
		destination.clone(),
		Some(10_000_000_000_000_000),
		Some(1_000_000_000_000_000),
		Some(Ratio::saturating_from_rational(lower, lower_base)),
		Some(Ratio::saturating_from_rational(upper, upper_base)),
	));
}

#[test]
fn adjust_stable_asset_basic_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(initial_stable_asset(AUSD, LDOT));
		let _ = Tokens::deposit(AUSD, &ALICE, 100_000_000_000u128);
		let swap_output = StableAssetWrapper::get_swap_output_amount(0, 0, 1, 1_000_000_000u128).unwrap();
		let (input, output) = StableAssetWrapper::swap(&ALICE, 0, 0, 1, 1_000_000_000u128, 0, 2).unwrap();
		assert_eq!(swap_output.dx, input);
		assert_eq!(swap_output.dy, output);

		let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
			pool_id: 0,
			stable_token_index: 0,
			account_id: CHARLIE,
		};
		let destination = DistributionDestination::StableAsset(distribution_to_stable_asset);

		// CASE#1. current rate=50%, less than target_min=65%, mint aUSD.
		// latest target=371_426_714_285_865/570_963_593_384_272=0.6505 ~= target_min=65%
		update_params(destination.clone(), 65, 70);

		let _ = Tokens::deposit(AUSD, &CHARLIE, 100_000_000_000_000);
		let _ = Tokens::deposit(LDOT, &CHARLIE, 100_000_000_000_000);
		StableAssetWrapper::mint(&CHARLIE, 0, vec![100_000_000_000_000, 100_000_000_000_000], 0).unwrap();

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
		System::assert_has_event(crate::mock::Event::HonzonDistribution(
			crate::Event::AdjustDestination {
				destination: destination.clone(),
				amount: ausd_mint as i128,
			},
		));
		// minted aUSD is add to `DistributedBalance`, and lp go to minter.
		assert_eq!(DistributedBalance::<Runtime>::get(&destination).unwrap(), ausd_mint);
		assert_eq!(Tokens::free_balance(STABLE_ASSET, &CHARLIE), stable_mint);

		// CASE#2. previous rate=0.6505 > target_max=62.5%, burn aUSD.
		// latest target=332_386_509_601_200/532_098_344_262_419=0.6246 ~= target_max=62.5%
		update_params(destination.clone(), 60, 625);
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		let stable_redeem = 38_865_249_121_853u128;
		let ausd_burn = 39_040_204_684_665u128;
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
			redeemer: CHARLIE,
			pool_id: 0,
			a: 3000,
			input_amount: stable_redeem,
			output_asset: AUSD,
			min_output_amount: 0,
			balances: vec![332_386_509_601_200, 199_999_000_000_165],
			total_supply: 532_098_344_262_419,
			fee_amount: 0,
			output_amount: ausd_burn,
		}));
		System::assert_has_event(crate::mock::Event::HonzonDistribution(
			crate::Event::AdjustDestination {
				destination: destination.clone(),
				amount: 0 as i128 - ausd_burn as i128,
			},
		));
		// redeemed aUSD is reduce from `DistributedBalance`, and lp also reduce from minter.
		assert_eq!(
			DistributedBalance::<Runtime>::get(&destination).unwrap(),
			ausd_mint - ausd_burn
		);
		assert_eq!(
			Tokens::free_balance(STABLE_ASSET, &CHARLIE),
			stable_mint - stable_redeem
		);

		// CASE#3. previous rate=0.6246 > target_max=50.5%, burn aUSD.
		// burned amount is less than `DistributedBalance`, success.
		// latest target=203459495144523/403458251840847=0.5042 ~= target_max=50.5%
		update_params(destination.clone(), 50, 505);
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		assert!(System::events().iter().any(|r| {
			matches!(
				r.event,
				Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle { .. })
					| crate::mock::Event::HonzonDistribution(crate::Event::AdjustDestination { .. })
			)
		}));

		// CASE#4. previous rate=0.5042 < target_min=62.5%, mint aUSD.
		// mint amount add distributed exceed capacity.
		// latest target=320_001_000_000_000/519_760_565_793_580=0.6156 ~= target_min=62.5%
		let distributed = DistributedBalance::<Runtime>::get(&destination).unwrap();
		assert_eq!(distributed, 3_458_495_144_523);
		let capacity = 120_000_000_000_000u128;
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(capacity),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(625, 1000)),
			Some(Ratio::saturating_from_rational(65, 100)),
		));
		System::reset_events();
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::Minted {
			minter: CHARLIE,
			pool_id: 0,
			a: 3000,
			input_amounts: vec![capacity - distributed, 0],
			min_output_amount: 0,
			balances: vec![320_001_000_000_000, 199_999_000_000_165],
			total_supply: 519_760_565_793_580,
			fee_amount: 0,
			output_amount: 116_302_313_952_733,
		}));
		assert_eq!(DistributedBalance::<Runtime>::get(&destination).unwrap(), capacity);

		// CASE#5. previous rate=0.6156 < target_min=90%, mint aUSD.
		// the target_min is too large than current rate, so the mint aUSD is abundant.
		// due to config, each time mint amount should not large than max_step.
		// latest target=1320001000000000/1505613028578396=0.8767 not reach target_min=90%.
		let max_step = 1_000_000_000_000_000u128;
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000_000_000_000_000),
			Some(max_step),
			Some(Ratio::saturating_from_rational(90, 100)),
			Some(Ratio::saturating_from_rational(95, 100)),
		));
		System::reset_events();
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		System::assert_has_event(crate::mock::Event::HonzonDistribution(
			crate::Event::AdjustDestination {
				destination: destination.clone(),
				amount: max_step as i128,
			},
		));
		// current DistributedBalance = last value(equal to capacity) + max_step.
		assert_eq!(
			DistributedBalance::<Runtime>::get(&destination).unwrap(),
			capacity + max_step
		);

		// CASE#6. previous rate=0.8767 > target_max=87.6%, burn aUSD.
		// but the burn amount(8aUSD) < MinimumAdjustAmount(10aUSD), so nothing happened.
		update_params(destination.clone(), 87, 876);
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		// DistributedBalance not changed, because there are none burn operation.
		let distributed = DistributedBalance::<Runtime>::get(&destination).unwrap();
		assert_eq!(distributed, capacity + max_step);
		assert_eq!(distributed, 1_120_000_000_000_000);
		System::assert_has_event(crate::mock::Event::HonzonDistribution(
			crate::Event::AdjustDestination {
				destination: destination.clone(),
				amount: 0i128,
			},
		));

		// CASE#7. previous rate=0.8767 > target_max=45%, burn aUSD.
		// burned amount is large than `DistributedBalance`, use `DistributedBalance`.
		let stable_amount = Tokens::free_balance(STABLE_ASSET, &CHARLIE);
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(5_000_000_000_000_000),
			Some(2_000_000_000_000_000), // make sure burn amount > max_step
			Some(Ratio::saturating_from_rational(40, 100)),
			Some(Ratio::saturating_from_rational(45, 100)),
		));
		System::reset_events();
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
			redeemer: CHARLIE,
			pool_id: 0,
			a: 3000,
			input_amount: distributed,
			output_asset: AUSD,
			min_output_amount: 0,
			balances: vec![185_618_430_327_950, 199_999_000_000_165],
			total_supply: 385613028578397,
			fee_amount: 0,
			output_amount: 1_134_382_569_672_050,
		}));
		assert_eq!(DistributedBalance::<Runtime>::get(&destination).unwrap(), 0);
		assert_eq!(
			stable_amount - Tokens::free_balance(STABLE_ASSET, &CHARLIE),
			distributed
		);

		// CASE#8. remove DistributedBalance, set cap to lower value, and first time mint > cap.
		// mint amount(3.4aUSD) < MinimumAdjustAmount(10aUSD). nothing happened.
		DistributedBalance::<Runtime>::remove(&destination);
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(5_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(877, 1000)),
			Some(Ratio::saturating_from_rational(95, 100)),
		));
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		assert_eq!(DistributedBalance::<Runtime>::get(&destination).unwrap(), 0);
	});
}

#[test]
fn redeem_stable_asset_works() {
	ExtBuilder::default().build().execute_with(|| {
		let (destination, ausd_mint) = first_mint_works();

		// update capacity lower than `DistributedBalance`, burn 10 aUSD
		let capacity = 40_000_000_000_000;
		let redeem_amount = 10_029_205_971_501;
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(40_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(60, 100)),
			Some(Ratio::saturating_from_rational(70, 100)),
		));
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
			redeemer: BOB,
			pool_id: 0,
			a: 3000,
			input_amount: ausd_mint - capacity,
			output_asset: AUSD,
			min_output_amount: 0,
			balances: vec![139_970_794_028_499, 100_000_000_000_000],
			total_supply: 239_914_704_134_299,
			fee_amount: 0,
			output_amount: redeem_amount,
		}));
		assert_eq!(
			DistributedBalance::<Runtime>::get(&destination).unwrap(),
			ausd_mint - redeem_amount
		);

		// capacity(25)<distributed(39.97), and burn amount(39.97-25=14.97)>MinimumAdjustAmount(10)
		let redeem_amount2 = 15_003_895_936_052;
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(25_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(60, 100)),
			Some(Ratio::saturating_from_rational(70, 100)),
		));
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
			redeemer: BOB,
			pool_id: 0,
			a: 3000,
			input_amount: 14_970_794_028_499,
			output_asset: AUSD,
			min_output_amount: 0,
			balances: vec![124_966_898_092_447, 100_000_000_000_000],
			total_supply: 224_943_910_105_800,
			fee_amount: 0,
			output_amount: redeem_amount2,
		}));
		assert_eq!(
			DistributedBalance::<Runtime>::get(&destination).unwrap(),
			ausd_mint - redeem_amount - redeem_amount2
		);

		// cap=0, distributed=25, burn=min(25-0, 125)~=25
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(0),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(60, 100)),
			Some(Ratio::saturating_from_rational(70, 100)),
		));
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
			redeemer: BOB,
			pool_id: 0,
			a: 3000,
			input_amount: 24_966_898_092_447,
			output_asset: AUSD,
			min_output_amount: 0,
			balances: vec![99_977_012_035_014, 100_000_000_000_000],
			total_supply: 199_977_012_013_353,
			fee_amount: 0,
			output_amount: 24_989_886_057_433,
		}));
		assert_eq!(DistributedBalance::<Runtime>::get(&destination).unwrap(), 0);

		assert_eq!(Tokens::free_balance(STABLE_ASSET, &BOB), 199_977_012_013_353);

		// cap > distributed(0), current(49.9%)>target_max(49%), burn amount=0
		assert_ok!(HonzonDistribution::update_params(
			Origin::root(),
			destination.clone(),
			Some(1_000_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(40, 100)),
			Some(Ratio::saturating_from_rational(49, 100)),
		));
		System::reset_events();
		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
		assert!(System::events().iter().all(|r| {
			!matches!(
				r.event,
				Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle { .. })
			)
		}));
	});
}

#[test]
fn remove_distribution_works() {
	ExtBuilder::default().build().execute_with(|| {
		let (destination, ausd_mint) = first_mint_works();

		// remove distribution
		assert_ok!(HonzonDistribution::remove_distribution(
			Origin::root(),
			destination.clone()
		));
		System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::RedeemedSingle {
			redeemer: BOB,
			pool_id: 0,
			a: 3000,
			input_amount: ausd_mint,
			output_asset: AUSD,
			min_output_amount: 0,
			balances: vec![99914704432597, 100000000000000],
			total_supply: 199914704134300,
			fee_amount: 0,
			output_amount: 50085295567403,
		}));
		assert_eq!(Tokens::free_balance(STABLE_ASSET, &BOB), 199_914_704_134_300);
		assert_eq!(DistributedBalance::<Runtime>::get(&destination), None);
		assert_eq!(DistributionDestinationParams::<Runtime>::get(&destination), None);
	});
}

fn first_mint_works() -> (DistributionDestination<AccountId>, Balance) {
	assert_ok!(initial_stable_asset(AUSD, LDOT));
	let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
		pool_id: 0,
		stable_token_index: 0,
		account_id: BOB,
	};
	let destination = DistributionDestination::StableAsset(distribution_to_stable_asset);

	// current rate=50%, less than target_min=65%, mint 50 aUSD.
	let ausd_mint = 50_000_000_000_000u128;
	update_params(destination.clone(), 60, 70);
	assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone()));
	System::assert_has_event(Event::StableAsset(nutsfinance_stable_asset::Event::Minted {
		minter: BOB,
		pool_id: 0,
		a: 3000,
		input_amounts: vec![ausd_mint, 0],
		min_output_amount: 0,
		balances: vec![150_000_000_000_000, 100_000_000_000_000],
		total_supply: 249_914_704_134_299,
		fee_amount: 0,
		output_amount: 49_914_704_134_299,
	}));
	System::assert_has_event(crate::mock::Event::HonzonDistribution(
		crate::Event::AdjustDestination {
			destination: destination.clone(),
			amount: ausd_mint as i128,
		},
	));
	assert_eq!(DistributedBalance::<Runtime>::get(&destination).unwrap(), ausd_mint);
	assert_eq!(Tokens::free_balance(STABLE_ASSET, &BOB), 249_914_704_134_299);

	(destination, ausd_mint)
}
