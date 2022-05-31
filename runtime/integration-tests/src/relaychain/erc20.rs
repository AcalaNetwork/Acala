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
use karura_runtime::AssetRegistry;
use module_support::EVM as EVMTrait;
use orml_traits::MultiCurrency;
use primitives::evm::EvmAddress;
use sp_core::{H256, U256};
use std::str::FromStr;
use xcm_emulator::TestExt;

pub fn erc20_address_0() -> EvmAddress {
	EvmAddress::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
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
	env_logger::init();
	TestNet::reset();

	fn sibling_reserve_account() -> AccountId {
		polkadot_parachain::primitives::Sibling::from(2001).into_account()
	}

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

	Karura::execute_with(|| {
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

		// transfer erc20 token to Sibling
		assert_ok!(XTokens::transfer_reserve(
			Origin::signed(CHARLIE.into()),
			CurrencyId::Erc20(erc20_address_0()),
			10_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2001),
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
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &sibling_reserve_account())
		);
	});

	Sibling::execute_with(|| {
		// Sibling will take (1, 2000, GeneralKey(Erc20(address))) as foreign asset
		assert_eq!(
			9_999_360_000_000,
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
			4_999_360_000_000,
			Currencies::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
	});

	Karura::execute_with(|| {
		println!(
			"{}",
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &AccountId::from(BOB))
		);
	});
}

#[test]
fn sibling_erc20_to_self_as_foreign_asset() {
	TestNet::reset();

	fn sibling_reserve_account() -> AccountId {
		polkadot_parachain::primitives::Sibling::from(2000).into_account()
	}

	Karura::execute_with(|| {
		let erc20_as_foreign_asset = CurrencyId::Erc20(erc20_address_0());
		// register Karura's erc20 as foreign asset
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X2(Parachain(2001), GeneralKey(erc20_as_foreign_asset.encode()))).into()),
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
		assert_ok!(XTokens::transfer_reserve(
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
			Currencies::free_balance(CurrencyId::Erc20(erc20_address_0()), &sibling_reserve_account())
		);
	});

	Karura::execute_with(|| {
		assert_eq!(
			9_999_360_000_000,
			Currencies::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
	});
}
