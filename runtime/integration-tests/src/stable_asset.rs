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

use crate::evm::alice_evm_addr;
use crate::payment::{with_fee_currency_call, with_fee_path_call, INFO};
use crate::setup::*;
use module_aggregated_dex::SwapPath;
use module_support::{ExchangeRate, Swap, SwapLimit, EVM as EVMTrait};
use primitives::{currency::AssetMetadata, evm::EvmAddress};
use sp_runtime::{
	traits::SignedExtension,
	transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use std::str::FromStr;

#[test]
fn stable_asset_mint_works() {
	ExtBuilder::default()
		.balances(vec![
			(
				// NetworkContractSource
				MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				LIQUID_CURRENCY,
				12_000_000_000 * dollar(NATIVE_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			let exchange_rate = Homa::current_exchange_rate();
			assert_eq!(exchange_rate, ExchangeRate::saturating_from_rational(1, 10)); // 0.1

			let pool_asset = CurrencyId::StableAssetPoolToken(0);
			assert_ok!(StableAsset::create_pool(
				Origin::root(),
				pool_asset,
				vec![RELAY_CHAIN_CURRENCY, LIQUID_CURRENCY], // assets
				vec![1u128, 1u128],                          // precisions
				10_000_000u128,                              // mint fee
				20_000_000u128,                              // swap fee
				50_000_000u128,                              // redeem fee
				1_000u128,                                   // initialA
				AccountId::from(BOB),                        // fee recipient
				AccountId::from(CHARLIE),                    // yield recipient
				1_000_000_000_000u128,                       // precision
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
			let account_id: AccountId = StableAssetPalletId::get().into_sub_account_truncating(0);

			assert_ok!(StableAsset::mint(
				Origin::signed(AccountId::from(ALICE)),
				0,
				vec![ksm_target_amount, lksm_target_amount],
				0u128
			));
			System::assert_last_event(Event::StableAsset(nutsfinance_stable_asset::Event::Minted {
				minter: AccountId::from(ALICE),
				pool_id: 0,
				a: 1000,
				input_amounts: vec![10_000_123u128, 10_000_456u128],
				min_output_amount: 0,
				balances: vec![10_000_123u128, 10_000_456u128],
				total_supply: 20_000_579u128,
				fee_amount: 20000,
				output_amount: 19_980_579u128,
			}));

			let ksm_balance = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &account_id);
			let lksm_balance = Currencies::free_balance(LIQUID_CURRENCY, &account_id);
			assert_eq!(ksm_target_amount, ksm_balance);

			#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
			let lksm_amount = 100_004_560u128;
			#[cfg(feature = "with-mandala-runtime")]
			let lksm_amount = 10_000_456u128;
			assert_eq!(lksm_amount, lksm_balance);

			let converted_lksm_balance = exchange_rate.checked_mul_int(lksm_balance).unwrap_or_default();
			#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
			assert_eq!(converted_lksm_balance == lksm_target_amount, true);
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(converted_lksm_balance < lksm_target_amount, true);
		});
}

#[test]
fn three_usd_pool_works() {
	let dollar = dollar(USD_CURRENCY);
	let alith = MockAddressMapping::get_account_id(&alice_evm_addr());
	ExtBuilder::default()
		.balances(vec![
			// alice() used to deploy erc20 contract
			(alice(), NATIVE_CURRENCY, 1_000_000 * dollar),
			(
				// NetworkContractSource
				MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar,
			),
			// alith used to mint 3USD.
			(alith.clone(), NATIVE_CURRENCY, 1_000_000_000 * dollar),
			(alith.clone(), USD_CURRENCY, 1_000_000_000 * dollar),
			(AccountId::from(ALICE), USD_CURRENCY, 1_000_000 * dollar),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar),
			(AccountId::from(BOB), NATIVE_CURRENCY, 1_000_000 * dollar),
		])
		.build()
		.execute_with(|| {
			// USDT is asset on Statemine
			assert_ok!(AssetRegistry::register_foreign_asset(
				Origin::root(),
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Parachain(1000),
							GeneralKey("USDT".as_bytes().to_vec().try_into().unwrap())
						)
					)
					.into()
				),
				Box::new(AssetMetadata {
					name: b"USDT".to_vec(),
					symbol: b"USDT".to_vec(),
					decimals: 12,
					minimal_balance: Balances::minimum_balance() / 10, // 10%
				})
			));
			// deposit USDT to alith, used for liquidity provider
			assert_ok!(Currencies::deposit(
				CurrencyId::ForeignAsset(0),
				&alith,
				1_000_000 * dollar
			));
			// deposit USDT to Bob, used for swap
			assert_ok!(Currencies::deposit(
				CurrencyId::ForeignAsset(0),
				&AccountId::from(BOB),
				1_000_000 * dollar
			));

			// USDC is Erc20 token
			deploy_erc20_contracts();
			let usdc: CurrencyId = CurrencyId::Erc20(erc20_address_0());
			let total_erc20 = 100_000_000_000_000_000_000_000u128;
			// alith has USDC when create Erc20 token
			assert_eq!(Currencies::free_balance(usdc, &alith), total_erc20);

			assert_ok!(EvmAccounts::claim_account(
				Origin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
			));
			assert_ok!(EvmAccounts::claim_account(
				Origin::signed(AccountId::from(BOB)),
				EvmAccounts::eth_address(&bob_key()),
				EvmAccounts::eth_sign(&bob_key(), &AccountId::from(BOB))
			));
			// transfer USDC erc20 to bob, used for swap
			<EVM as EVMTrait<AccountId>>::set_origin(alith.clone());
			assert_ok!(Currencies::transfer(
				Origin::signed(alith.clone()),
				sp_runtime::MultiAddress::Id(AccountId::from(BOB)),
				usdc,
				10 * dollar,
			));
			assert_ok!(Currencies::transfer(
				Origin::signed(alith.clone()),
				sp_runtime::MultiAddress::Id(AccountId::from(ALICE)),
				usdc,
				10 * dollar,
			));
			assert_eq!(Currencies::free_balance(usdc, &AccountId::from(BOB)), 10 * dollar);
			assert_eq!(Currencies::free_balance(usdc, &bob()), 10 * dollar);
			assert_eq!(Currencies::free_balance(usdc, &AccountId::from(ALICE)), 10 * dollar);
			assert_eq!(Currencies::free_balance(usdc, &alice()), 10 * dollar);

			let pool_asset = CurrencyId::StableAssetPoolToken(0);
			assert_ok!(StableAsset::create_pool(
				Origin::root(),
				pool_asset,
				vec![
					CurrencyId::ForeignAsset(0), // PoolTokenIndex=0
					usdc,                        // PoolTokenIndex=1
					USD_CURRENCY                 // PoolTokenIndex=2
				], // assets
				vec![1u128, 1u128, 1u128], // precisions
				10_000_000u128,            // mint fee
				20_000_000u128,            // swap fee
				50_000_000u128,            // redeem fee
				1_000u128,                 // initialA
				AccountId::from(BOB),      // fee recipient
				AccountId::from(CHARLIE),  // yield recipient
				1_000_000_000_000u128,     // precision
			));

			let asset_metadata = AssetMetadata {
				name: b"Three USD Pool".to_vec(),
				symbol: b"3USD".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			};
			assert_ok!(AssetRegistry::register_stable_asset(
				RawOrigin::Root.into(),
				Box::new(asset_metadata.clone())
			));

			assert_ok!(StableAsset::mint(
				Origin::signed(alith.clone()),
				0,
				vec![1000 * dollar, 1000 * dollar, 1000 * dollar],
				0u128
			));
			System::assert_last_event(Event::StableAsset(nutsfinance_stable_asset::Event::Minted {
				minter: alith,
				pool_id: 0,
				a: 1000,
				input_amounts: vec![1000 * dollar, 1000 * dollar, 1000 * dollar],
				min_output_amount: 0,
				balances: vec![1000 * dollar, 1000 * dollar, 1000 * dollar],
				total_supply: 3000 * dollar,
				fee_amount: 3 * dollar,
				output_amount: 2_997_000_000_000_000,
			}));

			// inject liquidity of AUSD to native token. Notice: USDC/USDT to AUSD liquidity is provided by
			// stable-asset pool, not by dex.
			assert_ok!(inject_liquidity(
				USD_CURRENCY,
				NATIVE_CURRENCY,
				1000 * dollar,
				10000 * dollar
			));
			assert_eq!(
				Dex::get_liquidity_pool(USD_CURRENCY, NATIVE_CURRENCY),
				(1000 * dollar, 10000 * dollar)
			);
			// Taiga(USDT, AUSD), Dex(AUSD, ACA)
			assert_ok!(AggregatedDex::update_aggregated_swap_paths(
				Origin::root(),
				vec![(
					(CurrencyId::ForeignAsset(0), NATIVE_CURRENCY),
					Some(vec![
						SwapPath::Taiga(0, 0, 2),
						SwapPath::Dex(vec![USD_CURRENCY, NATIVE_CURRENCY])
					])
				),]
			));
			// Taiga(USDC, AUSD), Dex(AUSD, ACA)
			assert_ok!(AggregatedDex::update_aggregated_swap_paths(
				Origin::root(),
				vec![(
					(usdc, NATIVE_CURRENCY),
					Some(vec![
						SwapPath::Taiga(0, 1, 2),
						SwapPath::Dex(vec![USD_CURRENCY, NATIVE_CURRENCY])
					])
				),]
			));
			#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
			let (amount1, amount2, amount3) = (9_940_060_348_765u128, 9_920_180_467_236u128, 9_920_507_587_087u128);
			#[cfg(feature = "with-mandala-runtime")]
			let (amount1, amount2, amount3) = (9_959_980_429_142u128, 9_940_040_907_508u128, 9_940_348_860_887u128);
			// USDC -> AUSD -> ACA
			assert_eq!(
				AcalaSwap::swap(
					&AccountId::from(BOB),
					usdc,
					NATIVE_CURRENCY,
					SwapLimit::ExactSupply(dollar, 0)
				),
				Ok((1_000_000_000_000, amount1))
			);
			// USDT -> AUSD -> ACA
			assert_eq!(
				AcalaSwap::swap(
					&AccountId::from(BOB),
					CurrencyId::ForeignAsset(0),
					NATIVE_CURRENCY,
					SwapLimit::ExactSupply(dollar, 0)
				),
				Ok((1_000_000_000_000, amount2))
			);
			// AUSD -> ACA
			assert_eq!(
				AcalaSwap::swap(
					&AccountId::from(BOB),
					USD_CURRENCY,
					NATIVE_CURRENCY,
					SwapLimit::ExactSupply(dollar, 0)
				),
				Ok((1_000_000_000_000, amount3))
			);

			// USDC: Erc20(contract) as fee token
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&with_fee_currency_call(usdc),
					&INFO,
					50
				)
			);
			assert!(System::events().iter().any(|r| matches!(
				r.event,
				Event::StableAsset(nutsfinance_stable_asset::Event::TokenSwapped {
					pool_id: 0,
					a: 1000,
					input_asset: _usdc,
					output_asset: USD_CURRENCY,
					..
				})
			)));
			assert!(System::events().iter().any(|r| matches!(
				r.event,
				// USD_CURRENCY, NATIVE_CURRENCY
				Event::Dex(module_dex::Event::Swap { .. })
			)));

			// USDT: ForeignAsset(0) as fee token
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&with_fee_currency_call(CurrencyId::ForeignAsset(0)),
					&INFO,
					50
				)
			);
			assert!(System::events().iter().any(|r| matches!(
				r.event,
				Event::StableAsset(nutsfinance_stable_asset::Event::TokenSwapped {
					pool_id: 0,
					a: 1000,
					input_asset: CurrencyId::ForeignAsset(0),
					output_asset: USD_CURRENCY,
					..
				})
			)));
			assert!(System::events().iter().any(|r| matches!(
				r.event,
				// USD_CURRENCY, NATIVE_CURRENCY
				Event::Dex(module_dex::Event::Swap { .. })
			)));

			// AUSD as fee token
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&with_fee_currency_call(USD_CURRENCY),
					&INFO,
					50
				)
			);
			#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
			let (amount1, amount2) = (227029695u128, 2250001739u128);
			#[cfg(feature = "with-mandala-runtime")]
			let (amount1, amount2) = (906308684u128, 9000001739u128);
			System::assert_has_event(Event::Dex(module_dex::Event::Swap {
				trader: AccountId::from(BOB),
				path: vec![USD_CURRENCY, NATIVE_CURRENCY],
				liquidity_changes: vec![amount1, amount2],
			}));

			// with_fee_path_call failed
			let invalid_swap_path = vec![
				vec![CurrencyId::ForeignAsset(0), USD_CURRENCY, NATIVE_CURRENCY],
				vec![CurrencyId::ForeignAsset(0), USD_CURRENCY],
				vec![CurrencyId::ForeignAsset(0), NATIVE_CURRENCY],
				vec![usdc, USD_CURRENCY, NATIVE_CURRENCY],
				vec![usdc, USD_CURRENCY],
				vec![usdc, NATIVE_CURRENCY],
			];
			for path in invalid_swap_path {
				assert_noop!(
					<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
						&AccountId::from(BOB),
						&with_fee_path_call(path),
						&INFO,
						50
					),
					TransactionValidityError::Invalid(InvalidTransaction::Payment)
				);
			}
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&with_fee_path_call(vec![USD_CURRENCY, NATIVE_CURRENCY]),
					&INFO,
					50
				)
			);
		});
}

pub fn deploy_erc20_contracts() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	assert_ok!(EVM::create(Origin::signed(alice()), code, 0, 2100_000, 100000, vec![]));
	assert_ok!(EVM::publish_free(Origin::root(), erc20_address_0()));
	assert_ok!(AssetRegistry::register_erc20_asset(
		Origin::root(),
		erc20_address_0(),
		100_000_000_000
	));
}

pub fn erc20_address_0() -> EvmAddress {
	EvmAddress::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
}

fn inject_liquidity(
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> Result<(), &'static str> {
	let alith = MockAddressMapping::get_account_id(&alice_evm_addr());
	let _ = Dex::enable_trading_pair(Origin::root(), currency_id_a, currency_id_b);
	Dex::add_liquidity(
		Origin::signed(alith),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		false,
	)?;
	Ok(())
}
