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

#[cfg(feature = "with-mandala-runtime")]
#[test]
fn test_mint() {
	use crate::setup::*;
	use primitives::currency::AssetMetadata;

	ExtBuilder::default()
		.balances(vec![
			(
				// NetworkContractSource
				MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(AccountId::from(ALICE), KSM, 1_000_000_000 * dollar(NATIVE_CURRENCY)),
			(AccountId::from(ALICE), LKSM, 12_000_000_000 * dollar(NATIVE_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			let pool_asset = CurrencyId::StableAssetPoolToken(0);
			assert_ok!(StableAsset::create_pool(
				Origin::root(),
				pool_asset,
				vec![KSM, LKSM],
				vec![1u128, 1u128],
				10_000_000u128,
				20_000_000u128,
				50_000_000u128,
				1_000u128,
				AccountId::from(BOB),
				AccountId::from(CHARLIE),
				1_000_000_000_000u128,
			));
			let asset_metadata = AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			};
			assert_ok!(AssetRegistry::register_stable_asset(
				RawOrigin::Root.into(),
				Box::new(asset_metadata.clone())
			));
			let ksm_target_amount = 10_000_123u128;
			let lksm_target_amount = 10_000_456u128;
			let exchange_rate = Homa::current_exchange_rate();
			let account_id: AccountId = StableAssetPalletId::get().into_sub_account_truncating(0);
			assert_ok!(StableAsset::mint(
				Origin::signed(AccountId::from(ALICE)),
				0,
				vec![ksm_target_amount, lksm_target_amount],
				0u128
			));
			assert_eq!(Currencies::free_balance(KSM, &account_id), ksm_target_amount);
			let lksm_balance = Currencies::free_balance(LKSM, &account_id);
			let converted_lksm_balance = exchange_rate.checked_mul_int(lksm_balance).unwrap_or_default();
			assert_eq!(converted_lksm_balance >= lksm_target_amount, true);
		});
}
