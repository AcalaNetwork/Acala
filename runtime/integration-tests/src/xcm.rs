// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

use crate::setup::*;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use karura_runtime::{
	FeePoolBootBalance, KarPerSecondAsBased, KaruraTreasuryAccount, KsmPerSecond, NativeTokenExistentialDeposit,
	TreasuryFeePoolPalletId,
};
use module_transaction_payment::PeriodUpdatedRateOfFungible;
use sp_runtime::{
	traits::{AccountIdConversion, UniqueSaturatedInto},
	MultiAddress,
};
use xcm_builder::FixedRateOfFungible;
use xcm_executor::{traits::*, Assets, Config};

#[cfg(feature = "with-karura-runtime")]
#[test]
fn trader_works() {
	// 4 instructions, each instruction cost 200_000_000
	let mut message = Xcm(vec![
		ReserveAssetDeposited((Parent, 100).into()),
		ClearOrigin,
		BuyExecution {
			fees: (Parent, 100).into(),
			weight_limit: Limited(100),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);
	let expect_weight: Weight = 800_000_000;
	let xcm_weight: Weight = <XcmConfig as Config>::Weigher::weight(&mut message).unwrap();
	assert_eq!(xcm_weight, expect_weight);

	// 0.16 * 800_000_000 = 128_000_000
	let asset: MultiAsset = (Parent, 130_000_000).into();
	let expect_result: MultiAsset = (Parent, 2_000_000).into();
	let assets: Assets = asset.into();

	// when no runtime upgrade, the newly PeriodUpdatedRateOfFungible will failed.
	ExtBuilder::default().build().execute_with(|| {
		let mut trader = FixedRateOfFungible::<KsmPerSecond, ()>::new();
		let result_assets = trader.buy_weight(xcm_weight, assets.clone()).unwrap();
		let result_asset: Vec<MultiAsset> = result_assets.into();
		assert_eq!(vec![expect_result.clone()], result_asset);

		let mut period_trader =
			PeriodUpdatedRateOfFungible::<Runtime, CurrencyIdConvert, KarPerSecondAsBased, ()>::new();
		let result_assets = period_trader.buy_weight(xcm_weight, assets.clone());
		assert_eq!(result_assets.is_err(), true);
	});

	// when runtime upgrade, the newly PeriodUpdatedRateOfFungible works as priority.
	ExtBuilder::default().build().execute_with(|| {
		let treasury_account = KaruraTreasuryAccount::get();
		let fee_account1: AccountId = TreasuryFeePoolPalletId::get().into_sub_account(KSM);
		let fee_account2: AccountId = TreasuryFeePoolPalletId::get().into_sub_account(KUSD);
		// FeePoolBootBalance set to 5 KAR = 50*ED, the treasury already got ED balance when startup.
		let ed = NativeTokenExistentialDeposit::get();
		let fee_balance = (FeePoolBootBalance::get() + ed) as u128;

		// make treasury richer: 100*ED
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			MultiAddress::Id(treasury_account.clone()),
			KAR,
			(ed * 50).unique_saturated_into(),
		));

		// runtime upgrade transfer native asset to fee account
		MockRuntimeUpgrade::on_runtime_upgrade();
		assert_eq!(Currencies::free_balance(KAR, &fee_account1), fee_balance);
		assert_eq!(Currencies::free_balance(KAR, &fee_account2), ed);
		assert_eq!(Currencies::free_balance(KAR, &treasury_account), ed);

		let mut period_trader =
			PeriodUpdatedRateOfFungible::<Runtime, CurrencyIdConvert, KarPerSecondAsBased, ()>::new();
		let result_assets = period_trader.buy_weight(xcm_weight, assets);
		let result_asset: Vec<MultiAsset> = result_assets.unwrap().into();
		assert_eq!(vec![expect_result.clone()], result_asset);
	});
}
