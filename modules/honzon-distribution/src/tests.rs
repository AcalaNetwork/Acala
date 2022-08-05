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
use frame_support::assert_ok;
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

		assert_ok!(HonzonDistribution::force_adjust(Origin::root(), destination.clone(),));
		System::assert_last_event(Event::StableAsset(nutsfinance_stable_asset::Event::Minted {
			minter: CHARLIE,
			pool_id: 0,
			a: 3000,
			input_amounts: vec![171425714285865, 0],
			min_output_amount: 0,
			balances: vec![371426714285865, 199999000000165],
			total_supply: 570963593384272,
			fee_amount: 0,
			output_amount: 170963593384189,
		}));

		let distributed = DistributedBalance::<Runtime>::get(&destination).unwrap();
		assert_eq!(distributed, 171425714285865);
	});
}
