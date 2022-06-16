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

//! Erc20 xcm transfer

use crate::relaychain::kusama_test_net::*;
use crate::setup::*;

use frame_support::assert_ok;
use karura_runtime::{AssetRegistry, Erc20HoldingAccount, KaruraTreasuryAccount};
use module_evm_accounts::EvmAddressMapping;
use module_support::EVM as EVMTrait;
use orml_traits::MultiCurrency;
use primitives::evm::EvmAddress;
use sp_core::{H256, U256};
use std::str::FromStr;
use xcm_emulator::TestExt;

pub const SIBLING_ID: u32 = 2002;
pub const KARURA_ID: u32 = 2000;

pub fn erc20_address_0() -> EvmAddress {
	EvmAddress::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

fn sibling_reserve_account() -> AccountId {
	polkadot_parachain::primitives::Sibling::from(SIBLING_ID).into_account_truncating()
}
fn karura_reserve_account() -> AccountId {
	polkadot_parachain::primitives::Sibling::from(KARURA_ID).into_account_truncating()
}

pub fn deploy_erc20_contracts() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	assert_ok!(EVM::create(Origin::signed(alice()), code, 0, 2100_000, 100000, vec![]));

	System::assert_last_event(Event::EVM(module_evm::Event::Created {
		from: EvmAddress::from_str("0xbf0b5a4099f0bf6c8bc4252ebec548bae95602ea").unwrap(),
		contract: erc20_address_0(),
		logs: vec![module_evm::Log {
			address: erc20_address_0(),
			topics: vec![
				H256::from_str("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef").unwrap(),
				H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000").unwrap(),
				H256::from_str("0x0000000000000000000000001000000000000000000000000000000000000001").unwrap(),
			],
			data: {
				let mut buf = [0u8; 32];
				U256::from(100_000_000_000_000_000_000_000u128).to_big_endian(&mut buf);
				H256::from_slice(&buf).as_bytes().to_vec()
			},
		}],
		used_gas: 1306611,
		used_storage: 15461,
	}));

	assert_ok!(EVM::publish_free(Origin::root(), erc20_address_0()));
	assert_ok!(AssetRegistry::register_erc20_asset(
		Origin::root(),
		erc20_address_0(),
		100_000_000_000
	));
}

#[test]
fn erc20_transfer_between_sibling() {
	TestNet::reset();

	Sibling::execute_with(|| {
		let erc20_as_foreign_asset = CurrencyId::Erc20(erc20_address_0());
		// register Karura's erc20 as foreign asset
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X2(Parachain(2000), GeneralKey(erc20_as_foreign_asset.encode()))).into()),
			Box::new(AssetMetadata {
				name: b"Karura USDC".to_vec(),
				symbol: b"kUSDC".to_vec(),
				decimals: 12,
				minimal_balance: Balances::minimum_balance() / 10, // 10%
			})
		));
	});

	let initial_native_amount = 1_000_000_000_000u128;
	let storage_fee = 6_400_000_000u128;

	Karura::execute_with(|| {
		let alith = MockAddressMapping::get_account_id(&alice_evm_addr());
		let total_erc20 = 100_000_000_000_000_000_000_000u128;
		let transfer_amount = 10 * dollar(NATIVE_CURRENCY);

		// used to deploy contracts
		assert_ok!(Currencies::deposit(
			NATIVE_CURRENCY,
			&alice(),
			1_000_000 * dollar(NATIVE_CURRENCY)
		));
		// when transfer erc20 cross chain, the origin `alith` is used to charge storage
		assert_ok!(Currencies::deposit(
			NATIVE_CURRENCY,
			&alith.clone(),
			initial_native_amount
		));
		// when withdraw sibling parachain account, the origin `sibling_reserve_account` is used to charge
		// storage
		assert_ok!(Currencies::deposit(
			NATIVE_CURRENCY,
			&sibling_reserve_account(),
			initial_native_amount
		));
		// when deposit to recipient, the origin is recipient `BOB`, and is used to charge storage.
		assert_ok!(Currencies::deposit(
			NATIVE_CURRENCY,
			&AccountId::from(BOB),
			initial_native_amount
		));
		// when xcm finished, deposit to treasury account, the origin is `treasury account`, and is used to
		// charge storage.
		assert_ok!(Currencies::deposit(
			NATIVE_CURRENCY,
			&KaruraTreasuryAccount::get(),
			initial_native_amount
		));

		deploy_erc20_contracts();

		// `transfer` invoked by `TransferReserveAsset` xcm instruction need to passing origin check.
		// In frontend/js, when issue xtokens extrinsic, it have `EvmSetOrigin` SignedExtra to `set_origin`.
		// In testcase, we're manual invoke `set_origin` here. because in erc20 xtokens transfer,
		// the `from` or `to` is not erc20 holding account. so we need make sure origin exists.
		<EVM as EVMTrait<AccountId>>::set_origin(alith.clone());

		assert_eq!(
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &alith),
			total_erc20
		);

		// transfer erc20 token to Sibling
		assert_ok!(XTokens::transfer(
			Origin::signed(alith.clone()),
			CurrencyId::Erc20(erc20_address_0()),
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(SIBLING_ID),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						},
					),
				)
				.into(),
			),
			1_000_000_000,
		));

		// using native token to charge storage fee
		assert_eq!(
			initial_native_amount - storage_fee,
			Currencies::free_balance(NATIVE_CURRENCY, &alith)
		);
		assert_eq!(
			total_erc20 - transfer_amount,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &alith)
		);
		assert_eq!(
			transfer_amount,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &sibling_reserve_account())
		);
		// initial_native_amount + ed
		assert_eq!(
			1_100_000_000_000,
			Currencies::free_balance(NATIVE_CURRENCY, &KaruraTreasuryAccount::get())
		);

		System::reset_events();
	});

	Sibling::execute_with(|| {
		// Sibling will take (1, 2000, GeneralKey(Erc20(address))) as foreign asset
		assert_eq!(
			9_999_067_600_000,
			Currencies::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);

		// transfer erc20 token back to Karura
		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			CurrencyId::ForeignAsset(0),
			5_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2000),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						},
					),
				)
				.into(),
			),
			1_000_000_000,
		));

		assert_eq!(
			4_999_067_600_000,
			Currencies::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
	});

	Karura::execute_with(|| {
		use karura_runtime::{Event, System};
		let erc20_holding_account = EvmAddressMapping::<Runtime>::get_account_id(&Erc20HoldingAccount::get());
		assert_eq!(
			5_000_000_000_000,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &sibling_reserve_account())
		);
		assert_eq!(
			4_990_676_000_000,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &AccountId::from(BOB))
		);
		assert_eq!(
			9_324_000_000,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &KaruraTreasuryAccount::get())
		);
		assert_eq!(
			0,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &erc20_holding_account)
		);
		// withdraw erc20 need charge storage fee
		assert_eq!(
			initial_native_amount - storage_fee,
			Currencies::free_balance(NATIVE_CURRENCY, &sibling_reserve_account())
		);
		// deposit erc20 need charge storage fee
		assert_eq!(
			initial_native_amount - storage_fee,
			Currencies::free_balance(NATIVE_CURRENCY, &AccountId::from(BOB))
		);
		// deposit reserve and unreserve storage fee, so the native token not changed.
		assert_eq!(
			1_100_000_000_000,
			Currencies::free_balance(NATIVE_CURRENCY, &KaruraTreasuryAccount::get())
		);

		// withdraw operation transfer from sibling parachain account to erc20 holding account
		System::assert_has_event(Event::Currencies(module_currencies::Event::Withdrawn {
			currency_id: CurrencyId::Erc20(erc20_address_0()),
			who: sibling_reserve_account(),
			amount: 5_000_000_000_000,
		}));
		// deposit operation transfer from erc20 holding account to recipient
		System::assert_has_event(Event::Currencies(module_currencies::Event::Deposited {
			currency_id: CurrencyId::Erc20(erc20_address_0()),
			who: AccountId::from(BOB),
			amount: 4_990_676_000_000,
		}));
		// TakeRevenue deposit from erc20 holding account to treasury account
		System::assert_has_event(Event::Currencies(module_currencies::Event::Deposited {
			currency_id: CurrencyId::Erc20(erc20_address_0()),
			who: KaruraTreasuryAccount::get(),
			amount: 9_324_000_000,
		}));
	});
}

#[test]
fn sibling_erc20_to_self_as_foreign_asset() {
	TestNet::reset();

	Karura::execute_with(|| {
		let erc20_as_foreign_asset = CurrencyId::Erc20(erc20_address_0());
		// register Karura's erc20 as foreign asset
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X2(Parachain(2002), GeneralKey(erc20_as_foreign_asset.encode()))).into()),
			Box::new(AssetMetadata {
				name: b"Sibling USDC".to_vec(),
				symbol: b"sUSDC".to_vec(),
				decimals: 12,
				minimal_balance: Balances::minimum_balance() / 10, // 10%
			})
		));
	});

	Sibling::execute_with(|| {
		let alith = MockAddressMapping::get_account_id(&alice_evm_addr());
		assert_ok!(Currencies::deposit(
			NATIVE_CURRENCY,
			&alice(),
			1_000_000 * dollar(NATIVE_CURRENCY)
		));
		assert_ok!(Currencies::deposit(
			NATIVE_CURRENCY,
			&alith.clone(),
			1_000_000 * dollar(NATIVE_CURRENCY)
		));

		deploy_erc20_contracts();

		// Erc20 claim account
		assert_ok!(EvmAccounts::claim_account(
			Origin::signed(AccountId::from(ALICE)),
			EvmAccounts::eth_address(&alice_key()),
			EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
		));

		<EVM as EVMTrait<AccountId>>::set_origin(alith.clone());

		// use Currencies `transfer` dispatch call to transfer erc20 token to bob.
		assert_ok!(Currencies::transfer(
			Origin::signed(alith),
			MultiAddress::Id(AccountId::from(CHARLIE)),
			CurrencyId::Erc20(erc20_address_0()),
			1_000_000_000_000_000
		));
		assert_eq!(
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &AccountId::from(CHARLIE)),
			1_000_000_000_000_000
		);

		// transfer erc20 token to Karura
		assert_ok!(XTokens::transfer(
			Origin::signed(CHARLIE.into()),
			CurrencyId::Erc20(erc20_address_0()),
			10_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2000),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						},
					),
				)
				.into(),
			),
			1_000_000_000,
		));

		assert_eq!(
			990_000_000_000_000,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &AccountId::from(CHARLIE))
		);
		assert_eq!(
			10_000_000_000_000,
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &karura_reserve_account())
		);
	});

	Karura::execute_with(|| {
		assert_eq!(
			9_999_067_600_000,
			Currencies::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
	});
}
