// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use frame_support::{
	assert_ok,
	dispatch::{DispatchClass, DispatchInfo, Pays},
};
use module_asset_registry::EvmErc20InfoMapping;
use module_evm_accounts::EvmAddressMapping;
use module_evm_bridge::EVMBridge;
use module_support::{EVMBridge as EVMBridgeT, Erc20InfoMapping, EVM as EVMTrait};
use primitives::{
	evm::{convert_decimals_to_evm, EvmAddress},
	Position, TradingPair,
};
use sp_core::{H256, U256};
use sp_runtime::{
	traits::SignedExtension,
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	Percent,
};
use std::str::FromStr;

pub fn erc20_address_0() -> EvmAddress {
	EvmAddress::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
}

pub fn erc20_address_1() -> EvmAddress {
	EvmAddress::from_str("0xec2a41295171e2028542ca82f1801ca1f356388b").unwrap()
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000002").unwrap()
}

pub fn lp_erc20() -> CurrencyId {
	TradingPair::from_currency_ids(
		CurrencyId::Erc20(erc20_address_0()),
		CurrencyId::Erc20(erc20_address_1()),
	)
	.unwrap()
	.dex_share_currency_id()
}

pub fn lp_erc20_aca() -> CurrencyId {
	TradingPair::from_currency_ids(CurrencyId::Erc20(erc20_address_0()), NATIVE_CURRENCY)
		.unwrap()
		.dex_share_currency_id()
}

pub fn lp_erc20_evm_address() -> EvmAddress {
	EvmErc20InfoMapping::<Runtime>::encode_evm_address(lp_erc20()).unwrap()
}

pub fn predeploy_token_contract() -> Vec<u8> {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../predeploy-contracts/resources/bytecodes.json")).unwrap();
	// get ACA contract
	assert_eq!(json[0][0].as_str().unwrap(), "ACA");
	hex::decode(json[0][2].as_str().unwrap().strip_prefix("0x").unwrap()).unwrap()
}

pub fn deploy_erc20_contracts() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	assert_ok!(EVM::create(
		RuntimeOrigin::signed(alice()),
		code.clone(),
		0,
		2100_000,
		100000,
		vec![]
	));

	System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
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
		used_gas: 1013342,
		used_storage: 14027,
	}));

	assert_ok!(AssetRegistry::register_erc20_asset(
		RuntimeOrigin::root(),
		erc20_address_0(),
		1
	));

	assert_ok!(EVM::create(
		RuntimeOrigin::signed(alice()),
		code,
		0,
		2100_000,
		100000,
		vec![]
	));

	System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
		from: EvmAddress::from_str("0xbf0b5a4099f0bf6c8bc4252ebec548bae95602ea").unwrap(),
		contract: erc20_address_1(),
		logs: vec![module_evm::Log {
			address: erc20_address_1(),
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
		used_gas: 1013342,
		used_storage: 14027,
	}));

	assert_ok!(AssetRegistry::register_erc20_asset(
		RuntimeOrigin::root(),
		erc20_address_1(),
		1
	));
}

fn deploy_contract(account: AccountId) -> Result<H160, DispatchError> {
	// pragma solidity ^0.5.0;
	//
	// contract Factory {
	//     Contract[] newContracts;
	//
	//     function createContract () public payable {
	//         Contract newContract = new Contract();
	//         newContracts.push(newContract);
	//     }
	// }
	//
	// contract Contract {}
	let contract = hex_literal::hex!("608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a40029").to_vec();

	EVM::create(RuntimeOrigin::signed(account), contract, 0, 1000000000, 100000, vec![])
		.map_or_else(|e| Err(e.error), |_| Ok(()))?;

	if let RuntimeEvent::EVM(module_evm::Event::<Runtime>::Created {
		from: _,
		contract: address,
		logs: _,
		used_gas: _,
		used_storage: _,
	}) = System::events().last().unwrap().event
	{
		Ok(address)
	} else {
		Err("deploy_contract failed".into())
	}
}

#[test]
fn dex_module_works_with_evm_contract() {
	let dex_share = CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Erc20(erc20_address_1()));

	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000_000_000 * dollar(NATIVE_CURRENCY)),
			(
				// evm alice
				MockAddressMapping::get_account_id(&alice_evm_addr()),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				USD_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar(NATIVE_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			deploy_erc20_contracts();
			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
			));

			// CurrencyId::DexShare(Erc20, Erc20)
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				10,
				100,
				100,
				1000,
				0,
			));

			<EVM as EVMTrait<AccountId>>::set_origin(MockAddressMapping::get_account_id(&alice_evm_addr()));
			assert_ok!(Dex::add_provision(
				RuntimeOrigin::signed(MockAddressMapping::get_account_id(&alice_evm_addr())),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				10,
				100,
			));
			assert_eq!(
				Dex::get_liquidity_pool(
					CurrencyId::Erc20(erc20_address_0()),
					CurrencyId::Erc20(erc20_address_1())
				),
				(0, 0)
			);
			assert_eq!(Currencies::total_issuance(dex_share), 0);
			assert_eq!(Currencies::free_balance(dex_share, &AccountId::from(ALICE)), 0);
			assert_eq!(
				Currencies::free_balance(dex_share, &MockAddressMapping::get_account_id(&alice_evm_addr())),
				0
			);

			// CurrencyId::DexShare(Erc20, Erc20)
			<EVM as EVMTrait<AccountId>>::set_origin(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr()));

			assert_ok!(Dex::add_provision(
				RuntimeOrigin::signed(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				100,
				1000,
			));
			assert_ok!(Dex::end_provisioning(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
			));
			assert_eq!(
				Dex::get_liquidity_pool(
					CurrencyId::Erc20(erc20_address_0()),
					CurrencyId::Erc20(erc20_address_1())
				),
				(110, 1100)
			);

			assert_eq!(Currencies::total_issuance(dex_share), 220);

			assert_ok!(Dex::claim_dex_share(
				RuntimeOrigin::signed(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())),
				EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr()),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
			));
			assert_eq!(
				Currencies::free_balance(
					dex_share,
					&EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())
				),
				220
			);

			assert_ok!(Dex::remove_liquidity(
				RuntimeOrigin::signed(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				1,
				0,
				0,
				false,
			));

			assert_eq!(
				Dex::get_liquidity_pool(
					CurrencyId::Erc20(erc20_address_0()),
					CurrencyId::Erc20(erc20_address_1())
				),
				(110, 1096)
			);

			assert_eq!(Currencies::total_issuance(dex_share), 219);

			assert_eq!(
				Currencies::free_balance(
					dex_share,
					&EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())
				),
				219
			);
		});
}

#[test]
fn test_evm_module() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY)),
			(bob(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			assert_eq!(Balances::free_balance(alice()), 1_000 * dollar(NATIVE_CURRENCY));
			assert_eq!(Balances::free_balance(bob()), 1_000 * dollar(NATIVE_CURRENCY));

			let alice_address = EvmAccounts::eth_address(&alice_key());
			let bob_address = EvmAccounts::eth_address(&bob_key());

			let contract = deploy_contract(alice()).unwrap();
			System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
				from: alice_address,
				contract,
				logs: vec![],
				used_gas: 132225,
				used_storage: 10367,
			}));

			assert_ok!(EVM::transfer_maintainer(
				RuntimeOrigin::signed(alice()),
				contract,
				bob_address
			));
			System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::TransferredMaintainer {
				contract,
				new_maintainer: bob_address,
			}));

			// test EvmAccounts Lookup
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 998_963_300_000_000);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(Balances::free_balance(alice()), 998_963_300_000_000);
			#[cfg(feature = "with-acala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 996_889_900_000_000);
			assert_eq!(Balances::free_balance(bob()), 1_000 * dollar(NATIVE_CURRENCY));
			let to = EvmAccounts::eth_address(&alice_key());
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(bob()),
				MultiAddress::Address20(to.0),
				NATIVE_CURRENCY,
				10 * dollar(NATIVE_CURRENCY)
			));
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_008_963_300_000_000);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_008_963_300_000_000);
			#[cfg(feature = "with-acala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_006_889_900_000_000);
			assert_eq!(
				Balances::free_balance(bob()),
				1_000 * dollar(NATIVE_CURRENCY) - 10 * dollar(NATIVE_CURRENCY)
			);
		});
}

#[test]
fn test_multicurrency_precompile_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				alice(), NATIVE_CURRENCY, 1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				// evm alice
				MockAddressMapping::get_account_id(&alice_evm_addr()),
				NATIVE_CURRENCY,
				(1_000_000_000_000_000_000u128),
			),
			(AccountId::from(ALICE), USD_CURRENCY, (1_000_000_000_000_000_000u128)),
			(AccountId::from(ALICE), RELAY_CHAIN_CURRENCY, (1_000_000_000_000_000_000u128)),
			(AccountId::from(BOB), USD_CURRENCY, (1_000_000_000_000_000_000u128)),
			(AccountId::from(BOB), RELAY_CHAIN_CURRENCY, (1_000_000_000_000_000_000u128)),
		])
		.build()
		.execute_with(|| {
			deploy_erc20_contracts();

			// Erc20
			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
			));
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				10,
				100,
				100,
				1000,
				0,
			));

			// CurrencyId::DexShare(Erc20, Erc20)
			<EVM as EVMTrait<AccountId>>::set_origin(MockAddressMapping::get_account_id(&alice_evm_addr()));
			assert_ok!(Dex::add_provision(
				RuntimeOrigin::signed(MockAddressMapping::get_account_id(&alice_evm_addr())),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				100,
				1000,
			));
			assert_ok!(Dex::end_provisioning(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
			));
			assert_eq!(
				Dex::get_liquidity_pool(
					CurrencyId::Erc20(erc20_address_0()),
					CurrencyId::Erc20(erc20_address_1())
				),
				(100, 1000)
			);

			assert_eq!(
				Currencies::total_issuance(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address_0()),
					DexShare::Erc20(erc20_address_1())
				)),
				200
			);

			assert_ok!(Dex::claim_dex_share(
				RuntimeOrigin::signed(MockAddressMapping::get_account_id(&alice_evm_addr())),
				MockAddressMapping::get_account_id(&alice_evm_addr()),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
			));
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Erc20(erc20_address_1())),
					&MockAddressMapping::get_account_id(&alice_evm_addr())
				),
				200
			);

			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				sp_runtime::MultiAddress::Id(TreasuryAccount::get()),
				NATIVE_CURRENCY,
				10 * dollar(NATIVE_CURRENCY)
			));
			// deploy mirrored token of the LP
			assert_ok!(EVM::create_predeploy_contract(
				RuntimeOrigin::root(),
				lp_erc20_evm_address(),
				predeploy_token_contract(),
				0,
				21000000,
				16500,
				vec![],
			));

			let invoke_context = module_support::InvokeContext {
				contract: lp_erc20_evm_address(),
				sender: alice_evm_addr(),
				origin: alice_evm_addr(),
			};

			assert_eq!(
				EVMBridge::<Runtime>::name(invoke_context),
				Ok(b"LP long string name, long string name, long string name, long string name, long string name - long string name, long string name, long string name, long string name, long string name"[..32].to_vec())
			);
			assert_eq!(
				EVMBridge::<Runtime>::symbol(invoke_context),
				Ok(b"LP_TestToken_TestToken".to_vec())
			);
			assert_eq!(
				EVMBridge::<Runtime>::decimals(invoke_context),
				Ok(17)
			);
			assert_eq!(
				EVMBridge::<Runtime>::total_supply(invoke_context),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::<Runtime>::balance_of(invoke_context, alice_evm_addr()),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::<Runtime>::total_supply(invoke_context),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::<Runtime>::balance_of(invoke_context, alice_evm_addr()),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::<Runtime>::transfer(invoke_context, bob_evm_addr(), 1),
				Ok(())
			);
			assert_eq!(
				EVMBridge::<Runtime>::balance_of(invoke_context, alice_evm_addr()),
				Ok(199)
			);
			assert_eq!(
				EVMBridge::<Runtime>::balance_of(invoke_context, bob_evm_addr()),
				Ok(1)
			);
		});
}

#[test]
fn should_not_kill_contract_on_transfer_all() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 2_000 * dollar(NATIVE_CURRENCY)),
			(bob(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			// pragma solidity ^0.5.0;
			//
			// contract Test {
			// 	 constructor() public payable {
			// 	 }
			// }
			let code = hex_literal::hex!("6080604052603e8060116000396000f3fe6080604052600080fdfea265627a7a72315820e816b34c9ce8a2446f3d059b4907b4572645fde734e31dabf5465c801dcb44a964736f6c63430005110032").to_vec();

			assert_ok!(EVM::create(RuntimeOrigin::signed(alice()), code, convert_decimals_to_evm(2 * dollar(NATIVE_CURRENCY)), 1000000000, 100000, vec![]));

			let contract = if let RuntimeEvent::EVM(module_evm::Event::Created{from: _, contract: address, logs: _, used_gas: _, used_storage: _}) = System::events().last().unwrap().event {
				address
			} else {
				panic!("deploy contract failed");
			};

			assert_eq!(Balances::free_balance(EvmAddressMapping::<Runtime>::get_account_id(&contract)), 2 * dollar(NATIVE_CURRENCY));

			#[cfg(feature = "with-ethereum-compatibility")]
			assert_eq!(Balances::free_balance(alice()), 1_998 * dollar(NATIVE_CURRENCY));
			#[cfg(all(not(feature = "with-ethereum-compatibility"), feature = "with-mandala-runtime"))]
			assert_eq!(Balances::free_balance(alice()), 1_996_993_800_000_000);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_996_993_800_000_000);
			#[cfg(feature = "with-acala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_994_981_400_000_000);

			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(EvmAddressMapping::<Runtime>::get_account_id(&contract)),
				alice().into(),
				NATIVE_CURRENCY,
				2 * dollar(NATIVE_CURRENCY)
			));

			assert_eq!(Balances::free_balance(EvmAddressMapping::<Runtime>::get_account_id(&contract)), 0);

			#[cfg(feature = "with-ethereum-compatibility")]
			assert_eq!(Balances::free_balance(alice()), 2000 * dollar(NATIVE_CURRENCY));
			#[cfg(all(not(feature = "with-ethereum-compatibility"), feature = "with-mandala-runtime"))]
			assert_eq!(Balances::free_balance(alice()), 1_998_993_800_000_000);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_998_993_800_000_000);
			#[cfg(feature = "with-acala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_996_981_400_000_000);

			// assert the contract account is not purged
			assert!(EVM::accounts(contract).is_some());
		});
}

#[test]
fn should_not_kill_contract_on_transfer_all_tokens() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY)),
			(alice(), USD_CURRENCY, 1_000 * dollar(USD_CURRENCY)),
			(bob(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			// pragma solidity ^0.5.0;
			//
			// contract Test {
			// 	 constructor() public payable {
			// 	 }
			//
			// 	 function kill() public {
			// 	     selfdestruct(address(0));
			// 	 }
			// }
			let code = hex_literal::hex!("608060405260848060116000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c806341c0e1b514602d575b600080fd5b60336035565b005b600073ffffffffffffffffffffffffffffffffffffffff16fffea265627a7a72315820ed64a7551098c4afc823bee1663309079d9cb8798a6bdd71be2cd3ccee52d98e64736f6c63430005110032").to_vec();
			assert_ok!(EVM::create(RuntimeOrigin::signed(alice()), code, 0, 1000000000, 100000, vec![]));

			let contract = if let RuntimeEvent::EVM(module_evm::Event::Created{from: _, contract: address, logs: _, used_gas: _, used_storage: _}) = System::events().last().unwrap().event {
				address
			} else {
				panic!("deploy contract failed");
			};

			assert!(EVM::accounts(contract).is_some());
			assert!(EVM::accounts(contract).unwrap().contract_info.is_some());
			let contract_account_id = EvmAddressMapping::<Runtime>::get_account_id(&contract);

			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(alice()),
				contract_account_id.clone().into(),
				USD_CURRENCY,
				2 * dollar(USD_CURRENCY)
			));

			assert_eq!(Currencies::free_balance(USD_CURRENCY, &alice()), 998 * dollar(USD_CURRENCY));
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &contract_account_id), 2 * dollar(USD_CURRENCY));
			assert_eq!(EVM::accounts(contract).unwrap().nonce, 1);
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(contract_account_id.clone()),
				alice().into(),
				USD_CURRENCY,
				2 * dollar(USD_CURRENCY)
			));
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &contract_account_id), 0);

			assert_eq!(Currencies::free_balance(USD_CURRENCY, &alice()), 1000 * dollar(USD_CURRENCY));

			// assert the contract account is not purged
			#[cfg(feature = "with-ethereum-compatibility")]
			assert_eq!(System::providers(&contract_account_id), 1);
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			assert_eq!(System::providers(&contract_account_id), 1);
			assert!(EVM::accounts(contract).is_some());

			// call kill
			assert_ok!(EVM::call(RuntimeOrigin::signed(alice()), contract.clone(), hex_literal::hex!("41c0e1b5").to_vec(), 0, 1000000000, 100000, vec![]));

			#[cfg(feature = "with-ethereum-compatibility")]
			assert_eq!(System::providers(&contract_account_id), 0);
			#[cfg(not(feature = "with-ethereum-compatibility"))]
			assert_eq!(System::providers(&contract_account_id), 0);

			// contract account should be gone
			assert_eq!(EVM::accounts(contract), None);

			// should be gone
			assert!(!System::account_exists(&contract_account_id));
		});
}

#[test]
fn test_evm_accounts_module() {
	ExtBuilder::default()
		.balances(vec![(bob(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY))])
		.build()
		.execute_with(|| {
			assert_eq!(Balances::free_balance(AccountId::from(ALICE)), 0);
			assert_eq!(Balances::free_balance(bob()), 1_000 * dollar(NATIVE_CURRENCY));
			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
			));
			System::assert_last_event(RuntimeEvent::EvmAccounts(module_evm_accounts::Event::ClaimAccount {
				account_id: AccountId::from(ALICE),
				evm_address: EvmAccounts::eth_address(&alice_key()),
			}));

			// claim another eth address
			assert_noop!(
				EvmAccounts::claim_account(
					RuntimeOrigin::signed(AccountId::from(ALICE)),
					EvmAccounts::eth_address(&alice_key()),
					EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
				),
				module_evm_accounts::Error::<Runtime>::AccountIdHasMapped
			);
			assert_noop!(
				EvmAccounts::claim_account(
					RuntimeOrigin::signed(AccountId::from(BOB)),
					EvmAccounts::eth_address(&alice_key()),
					EvmAccounts::eth_sign(&alice_key(), &AccountId::from(BOB))
				),
				module_evm_accounts::Error::<Runtime>::EthAddressHasMapped
			);

			// evm padded address will transfer_all to origin.
			assert_eq!(Balances::free_balance(bob()), 1_000 * dollar(NATIVE_CURRENCY));
			assert_eq!(Balances::free_balance(&AccountId::from(BOB)), 0);
			assert_eq!(System::providers(&bob()), 1);
			assert_eq!(System::providers(&AccountId::from(BOB)), 0);
			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				EvmAccounts::eth_address(&bob_key()),
				EvmAccounts::eth_sign(&bob_key(), &AccountId::from(BOB))
			));
			assert_eq!(System::providers(&bob()), 0);
			assert_eq!(System::providers(&AccountId::from(BOB)), 1);
			assert_eq!(Balances::free_balance(bob()), 0);
			assert_eq!(
				Balances::free_balance(&AccountId::from(BOB)),
				1_000 * dollar(NATIVE_CURRENCY)
			);
		});
}

#[test]
fn test_default_evm_address_in_evm_accounts_module() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000_000_000 * dollar(NATIVE_CURRENCY)),
			(
				// evm alice
				MockAddressMapping::get_account_id(&alice_evm_addr()),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			deploy_erc20_contracts();

			assert!(EvmAccounts::evm_addresses(AccountId::from(ALICE)).is_none());
			assert!(EvmAccounts::evm_addresses(AccountId::from(BOB)).is_none());

			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
			));
			assert!(EvmAccounts::evm_addresses(AccountId::from(ALICE)).is_some());

			// get_or_create_evm_address
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())),
				sp_runtime::MultiAddress::Id(AccountId::from(BOB)),
				CurrencyId::Erc20(erc20_address_0()),
				10
			));

			assert!(EvmAccounts::evm_addresses(AccountId::from(BOB)).is_some());
			assert!(!System::account_exists(&AccountId::from(BOB)));

			// BOB claim eth address
			assert_noop!(
				EvmAccounts::claim_account(
					RuntimeOrigin::signed(AccountId::from(BOB)),
					EvmAccounts::eth_address(&bob_key()),
					EvmAccounts::eth_sign(&bob_key(), &AccountId::from(BOB))
				),
				module_evm_accounts::Error::<Runtime>::AccountIdHasMapped
			);

			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				sp_runtime::MultiAddress::Id(AccountId::from(BOB)),
				NATIVE_CURRENCY,
				10 * dollar(NATIVE_CURRENCY)
			));
			assert!(System::account_exists(&AccountId::from(BOB)));

			// on killed will remove the claim map.
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				sp_runtime::MultiAddress::Id(AccountId::from(ALICE)),
				NATIVE_CURRENCY,
				10 * dollar(NATIVE_CURRENCY)
			));
			assert!(!System::account_exists(&AccountId::from(BOB)));
			assert!(EvmAccounts::evm_addresses(AccountId::from(BOB)).is_none());

			// BOB claim eth address succeed.
			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				EvmAccounts::eth_address(&bob_key()),
				EvmAccounts::eth_sign(&bob_key(), &AccountId::from(BOB))
			));
		});
}

#[test]
fn transaction_payment_module_works_with_evm_contract() {
	let erc20_token = CurrencyId::Erc20(erc20_address_0());
	let dex_share = CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Token(NATIVE_TOKEN_SYMBOL));
	let sub_account: AccountId = TransactionPaymentPalletId::get().into_sub_account_truncating(erc20_token);
	let dollar = dollar(NATIVE_CURRENCY);
	let alice_evm_account = MockAddressMapping::get_account_id(&alice_evm_addr());
	let ed = NativeTokenExistentialDeposit::get(); // 100_000_000_000

	// new account
	let empty_account = AccountId::new([1u8; 32]);
	let empty_address = H160::from_slice(&[1u8; 20]);
	let empty_address_account = MockAddressMapping::get_account_id(&empty_address);

	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000_000_000 * dollar),
			(
				// evm alice
				alice_evm_account.clone(),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar,
			),
			(AccountId::from(ALICE), USD_CURRENCY, 1_000_000_000 * dollar),
			(AccountId::from(ALICE), RELAY_CHAIN_CURRENCY, 1_000_000_000 * dollar),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar),
			(AccountId::from(BOB), RELAY_CHAIN_CURRENCY, 1_000_000_000 * dollar),
		])
		.build()
		.execute_with(|| {
			deploy_erc20_contracts();
			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
			));

			// CurrencyId::DexShare(Erc20, ACA)
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				erc20_token,
				NATIVE_CURRENCY,
				10 * dollar,
				100 * dollar,
				100 * dollar,
				1000 * dollar,
				0,
			));

			<EVM as EVMTrait<AccountId>>::set_origin(alice_evm_account.clone());
			assert_ok!(Dex::add_provision(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				NATIVE_CURRENCY,
				10 * dollar,
				100 * dollar,
			));
			assert_eq!(Dex::get_liquidity_pool(erc20_token, NATIVE_CURRENCY,), (0, 0));
			assert_eq!(Currencies::total_issuance(dex_share), 0);
			assert_eq!(Currencies::free_balance(dex_share, &AccountId::from(ALICE)), 0);
			assert_eq!(Currencies::free_balance(dex_share, &alice_evm_account), 0);

			// CurrencyId::DexShare(Erc20, ACA)
			<EVM as EVMTrait<AccountId>>::set_origin(alice_evm_account.clone());
			assert_ok!(Dex::add_provision(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				NATIVE_CURRENCY,
				100 * dollar,
				1000 * dollar,
			));
			assert_ok!(Dex::end_provisioning(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				erc20_token,
				NATIVE_CURRENCY,
			));
			assert_eq!(
				Dex::get_liquidity_pool(erc20_token, NATIVE_CURRENCY,),
				(110 * dollar, 1100 * dollar)
			);

			// The order of dex share is related
			assert_eq!(Currencies::total_issuance(dex_share), 0);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::DexShare(
					DexShare::Token(NATIVE_TOKEN_SYMBOL),
					DexShare::Erc20(erc20_address_0()),
				)),
				2200 * dollar
			);
			assert_eq!(Currencies::total_issuance(lp_erc20_aca()), 2200 * dollar);

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				MultiAddress::Id(TreasuryAccount::get()),
				NATIVE_CURRENCY,
				(100 * dollar).try_into().unwrap()
			));

			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &sub_account), 0);
			assert_eq!(Currencies::free_balance(erc20_token, &sub_account), 0);

			// enable Erc20 token as fee pool token
			assert_ok!(TransactionPayment::enable_charge_fee_pool(
				RuntimeOrigin::root(),
				erc20_token,
				5 * dollar,
				Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar),
			));

			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &sub_account), 5 * dollar);
			// erc20 minimum_balance is 0
			assert_eq!(Currencies::free_balance(erc20_token, &sub_account), 0);

			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				MultiAddress::Id(empty_account.clone()),
				erc20_token,
				1
			));
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				MultiAddress::Address20(empty_address.0),
				erc20_token,
				1
			));
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &empty_account), 0);
			assert_eq!(Currencies::free_balance(erc20_token, &empty_account), 1);
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &empty_address_account), 0);
			assert_eq!(Currencies::free_balance(erc20_token, &empty_address_account), 1);

			// transfer erc20 to user so that user can charge erc20 as tx fee.
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				MultiAddress::Id(empty_account.clone()),
				erc20_token,
				5 * dollar
			));
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				MultiAddress::Address20(empty_address.0),
				erc20_token,
				5 * dollar
			));
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &empty_account), 0);
			assert_eq!(Currencies::free_balance(erc20_token, &empty_account), 5 * dollar + 1);
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &empty_address_account), 0);
			assert_eq!(
				Currencies::free_balance(erc20_token, &empty_address_account),
				5 * dollar + 1
			);

			let len = 150 as u32;
			let call: &<Runtime as frame_system::Config>::RuntimeCall =
				&RuntimeCall::Currencies(module_currencies::Call::transfer {
					dest: MultiAddress::Id(AccountId::from(BOB)),
					currency_id: erc20_token,
					amount: 1,
				});
			let info: DispatchInfo = DispatchInfo {
				weight: Weight::from_parts(100, 0),
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let fee = module_transaction_payment::Pallet::<Runtime>::compute_fee(len, &info, 0);
			assert_debug_snapshot!(fee, @"2500000934");

			let surplus_perc = Percent::from_percent(50); // CustomFeeSurplus
			let fee_surplus = surplus_perc.mul_ceil(fee);
			let fee = fee + fee_surplus;
			assert_debug_snapshot!(fee, @"3750001401");

			// empty_account use payment non wrapped call to charge fee by erc20 fee pool.
			assert_eq!(Currencies::free_balance(erc20_token, &sub_account), 0);
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&empty_account,
					call,
					&info,
					len as usize,
				)
			);
			let erc20_fee = Currencies::free_balance(erc20_token, &sub_account);
			#[cfg(feature = "with-mandala-runtime")]
			assert_debug_snapshot!(erc20_fee, @"10386329737");
			#[cfg(feature = "with-karura-runtime")]
			assert_debug_snapshot!(erc20_fee, @"10407164903");
			#[cfg(feature = "with-acala-runtime")]
			assert_debug_snapshot!(erc20_fee, @"10407164903");

			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &sub_account),
				5 * dollar - (fee + ed)
			);
			assert_eq!(
				Currencies::free_balance(erc20_token, &empty_account),
				5 * dollar + 1 - erc20_fee
			);
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &empty_account), ed);

			// empty_address use payment non wrapped call to charge fee by erc20 fee pool.
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&empty_address_account,
					call,
					&info,
					len as usize,
				)
			);
			assert_eq!(Currencies::free_balance(erc20_token, &sub_account), erc20_fee * 2);
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &sub_account),
				5 * dollar - (fee + ed) * 2
			);
			assert_eq!(
				Currencies::free_balance(erc20_token, &empty_address_account),
				5 * dollar + 1 - erc20_fee
			);
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &empty_address_account), ed);

			// empty_account use payment `with_fee_currency` call to charge fee by erc20 fee pool.
			let with_fee_call: <Runtime as module_transaction_payment::Config>::RuntimeCall =
				RuntimeCall::TransactionPayment(module_transaction_payment::Call::with_fee_currency {
					currency_id: erc20_token,
					call: Box::new(call.clone()),
				});
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&empty_account,
					&with_fee_call,
					&info,
					len as usize,
				)
			);

			let erc20_with_fee = Currencies::free_balance(erc20_token, &sub_account) - erc20_fee * 2;
			#[cfg(feature = "with-karura-runtime")]
			assert_debug_snapshot!(erc20_with_fee, @"376162722");
			#[cfg(feature = "with-acala-runtime")]
			assert_debug_snapshot!(erc20_with_fee, @"376162722");
			#[cfg(feature = "with-mandala-runtime")]
			assert_debug_snapshot!(erc20_with_fee, @"375409643");

			let native_with_fee = 5 * dollar - (fee + ed) * 2 - Currencies::free_balance(NATIVE_CURRENCY, &sub_account);
			#[cfg(feature = "with-karura-runtime")]
			assert_debug_snapshot!(native_with_fee, @"3750001401");
			#[cfg(feature = "with-acala-runtime")]
			assert_debug_snapshot!(native_with_fee, @"3750001401");
			#[cfg(feature = "with-mandala-runtime")]
			assert_debug_snapshot!(native_with_fee, @"3750001401");

			// empty_address use payment `with_fee_currency` call to charge fee by erc20 fee pool.
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&empty_address_account,
					&with_fee_call,
					&info,
					len as usize,
				)
			);
			assert_eq!(
				Currencies::free_balance(erc20_token, &sub_account),
				erc20_fee * 2 + erc20_with_fee * 2
			);
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &sub_account),
				5 * dollar - (fee + ed) * 2 - native_with_fee * 2
			);
		});
}

#[test]
fn create_contract_use_none_native_token_to_charge_storage() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), USD_CURRENCY, 10000 * dollar(USD_CURRENCY)),
			(AccountId::from(ALICE), NATIVE_CURRENCY, 10000 * dollar(NATIVE_CURRENCY)),
			(AccountId::from(BOB), USD_CURRENCY, 10000 * dollar(USD_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				USD_CURRENCY,
				NATIVE_CURRENCY,
				100 * dollar(USD_CURRENCY),
				1000 * dollar(NATIVE_CURRENCY),
				0,
				false
			));
			assert_eq!(
				(100 * dollar(USD_CURRENCY), 1000 * dollar(NATIVE_CURRENCY)),
				Dex::get_liquidity_pool(USD_CURRENCY, NATIVE_CURRENCY)
			);
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				sp_runtime::MultiAddress::Id(TreasuryAccount::get()),
				NATIVE_CURRENCY,
				100 * dollar(NATIVE_CURRENCY)
			));
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				sp_runtime::MultiAddress::Id(TreasuryAccount::get()),
				USD_CURRENCY,
				100 * dollar(USD_CURRENCY)
			));
			assert_ok!(TransactionPayment::enable_charge_fee_pool(
				RuntimeOrigin::root(),
				USD_CURRENCY,
				50 * dollar(NATIVE_CURRENCY),
				Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar(NATIVE_CURRENCY)),
			));

			#[cfg(feature = "with-karura-runtime")]
			{
				let contract_address = deploy_contract(AccountId::from(BOB)).unwrap();
				System::assert_has_event(RuntimeEvent::Balances(pallet_balances::Event::Reserved {
					who: AccountId::from(BOB),
					amount: 10_000_000_000_000,
				}));
				System::assert_has_event(RuntimeEvent::Balances(pallet_balances::Event::ReserveRepatriated {
					from: AccountId::from(BOB),
					to: MockAddressMapping::get_account_id(&contract_address),
					amount: 1_036_700_000_000,
					destination_status: frame_support::traits::BalanceStatus::Reserved,
				}));
				System::assert_has_event(RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
					who: AccountId::from(BOB),
					amount: 8_963_300_000_000,
				}));
				System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
					from: EvmAddress::from_str("0x414d1f1c39e8357acfa07e8aac63cc5da8f9ca4d").unwrap(),
					contract: EvmAddress::from_str("0xa764c25fe7641aeb21ac08118fa343093b9cb30d").unwrap(),
					logs: vec![],
					used_gas: 132225,
					used_storage: 10367,
				}));
			}
		});
}

#[test]
fn evm_limits() {
	ExtBuilder::default().build().execute_with(|| {
		assert_debug_snapshot!(runtime_common::EvmLimits::<Runtime>::max_gas_limit(), @"33321436");
		assert_debug_snapshot!(runtime_common::EvmLimits::<Runtime>::max_storage_limit(), @"3670016");
	});
}

#[test]
fn honzon_works_with_evm_contract() {
	let erc20_token = CurrencyId::Erc20(erc20_address_0());
	let alice_evm_account = MockAddressMapping::get_account_id(&alice_evm_addr());

	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000_000_000 * dollar(NATIVE_CURRENCY)),
			(
				// evm alice
				alice_evm_account.clone(),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				// evm alice
				alice_evm_account.clone(),
				USD_CURRENCY,
				1_000_000_000 * dollar(USD_CURRENCY),
			),
			(
				alice_evm_account.clone(),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			deploy_erc20_contracts();

			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
				erc20_token,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10000 * dollar(NATIVE_CURRENCY)),
			));

			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10000 * dollar(NATIVE_CURRENCY)),
			));

			assert_eq!(
				Loans::positions(USD_CURRENCY, alice_evm_account.clone()),
				Position {
					collateral: 0,
					debit: 0
				}
			);
			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: 0,
					debit: 0
				}
			);

			assert_eq!(
				Loans::total_positions(erc20_token),
				Position {
					collateral: 0,
					debit: 0
				}
			);

			set_oracle_price(vec![
				(erc20_token, Price::saturating_from_rational(2, 1)), // 2 usd
				(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(10, 1)), // 10 usd
			]);

			// erc20 decimals is 17
			let collateral_value = MinimumDebitValue::get() * 1_000_000; // 10 token, 10^18
			let min_debit_value = DefaultDebitExchangeRate::get()
				.reciprocal()
				.map(|n| n.saturating_mul_int(MinimumDebitValue::get()))
				.unwrap();

			<EVM as EVMTrait<AccountId>>::set_origin(alice_evm_account.clone());
			// 1.Honzon::adjust_loan
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				collateral_value as i128,
				min_debit_value as i128
			));

			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: collateral_value,
					debit: min_debit_value
				}
			);

			// collateral = 0
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				0,
				min_debit_value as i128
			));

			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: collateral_value,
					debit: 2 * min_debit_value
				}
			);

			// debit = 0
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				2 * collateral_value as i128,
				0,
			));

			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: 3 * collateral_value,
					debit: 2 * min_debit_value
				}
			);

			// 2.Honzon::adjust_loan_by_debit_value
			// withdraws debit
			assert_ok!(Honzon::adjust_loan_by_debit_value(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				0,
				-3 * min_debit_value as i128
			));
			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: 3 * collateral_value,
					debit: 0
				}
			);

			// Honzon::adjust_loan_by_debit_value
			// withdraws collateral
			assert_ok!(Honzon::adjust_loan_by_debit_value(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				-3 * collateral_value as i128,
				0,
			));
			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: 0,
					debit: 0
				}
			);

			// 3.Honzon::transfer_debit
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				collateral_value as i128,
				min_debit_value as i128
			));
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				RELAY_CHAIN_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY) as i128,
				min_debit_value as i128
			));
			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: collateral_value,
					debit: min_debit_value
				}
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, alice_evm_account.clone()),
				Position {
					collateral: 100 * dollar(RELAY_CHAIN_CURRENCY),
					debit: min_debit_value
				}
			);

			// Honzon::transfer_debit
			assert_ok!(Honzon::transfer_debit(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				RELAY_CHAIN_CURRENCY,
				min_debit_value
			));
			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: collateral_value,
					debit: 0
				}
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, alice_evm_account.clone()),
				Position {
					collateral: 100 * dollar(RELAY_CHAIN_CURRENCY),
					debit: 2 * min_debit_value
				}
			);
			assert_ok!(Honzon::transfer_debit(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				RELAY_CHAIN_CURRENCY,
				erc20_token,
				2 * min_debit_value
			));
			assert_eq!(
				Loans::positions(erc20_token, alice_evm_account.clone()),
				Position {
					collateral: collateral_value,
					debit: 2 * min_debit_value
				}
			);

			// 4.Honzon::expand_position_collateral
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				erc20_token,
				USD_CURRENCY,
				10,
				100,
				100,
				1000,
				0,
			));

			assert_ok!(Dex::add_provision(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				USD_CURRENCY,
				1000 * collateral_value,
				100 * min_debit_value
			));
			assert_ok!(Dex::end_provisioning(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				erc20_token,
				USD_CURRENCY,
			));
			assert_ok!(Honzon::expand_position_collateral(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				min_debit_value,
				collateral_value
			));

			// 5.Honzon::shrink_position_debit
			assert_ok!(Honzon::shrink_position_debit(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				erc20_token,
				collateral_value,
				min_debit_value / 10
			));
		});
}

#[test]
fn transaction_payment_module_charge_erc20_pool() {
	let erc20_token = CurrencyId::Erc20(erc20_address_0());
	let sub_account: AccountId = TransactionPaymentPalletId::get().into_sub_account_truncating(erc20_token);
	let dollar = dollar(NATIVE_CURRENCY);
	let alice_evm_account = MockAddressMapping::get_account_id(&alice_evm_addr());
	let account_with_erc20 = AccountId::new([1u8; 32]);

	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1_000_000_000 * dollar),
			(
				// evm alice
				alice_evm_account.clone(),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar,
			),
		])
		.build()
		.execute_with(|| {
			deploy_erc20_contracts();
			assert_ok!(EvmAccounts::claim_account(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE))
			));

			assert_ok!(Dex::enable_trading_pair(
				RuntimeOrigin::root(),
				NATIVE_CURRENCY,
				erc20_token
			));

			<EVM as EVMTrait<AccountId>>::set_origin(alice_evm_account.clone());
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				NATIVE_CURRENCY,
				erc20_token,
				1000 * dollar,
				100 * dollar,
				0,
				false
			));
			assert_eq!(
				Dex::get_liquidity_pool(NATIVE_CURRENCY, erc20_token),
				(1000 * dollar, 100 * dollar)
			);

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				MultiAddress::Id(TreasuryAccount::get()),
				NATIVE_CURRENCY,
				(100 * dollar).try_into().unwrap()
			));

			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &sub_account), 0);
			assert_eq!(Currencies::free_balance(erc20_token, &sub_account), 0);

			// enable Erc20 token as fee pool token
			assert_ok!(TransactionPayment::enable_charge_fee_pool(
				RuntimeOrigin::root(),
				erc20_token,
				5 * dollar,
				2 * dollar,
			));

			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &sub_account), 5 * dollar);
			// erc20 minimum_balance is 0
			assert_eq!(Currencies::free_balance(erc20_token, &sub_account), 0);

			// transfer erc20 token to account_with_erc20
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(alice_evm_account.clone()),
				MultiAddress::Id(account_with_erc20.clone()),
				erc20_token,
				dollar
			));
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &account_with_erc20), 0);
			assert_eq!(Currencies::free_balance(erc20_token, &account_with_erc20), dollar);

			let len = 150 as u32;
			let call: &<Runtime as frame_system::Config>::RuntimeCall =
				&RuntimeCall::System(frame_system::Call::remark {
					remark: "0x1234".into(),
				});
			let with_fee_call: <Runtime as module_transaction_payment::Config>::RuntimeCall =
				RuntimeCall::TransactionPayment(module_transaction_payment::Call::with_fee_currency {
					currency_id: erc20_token,
					call: Box::new(call.clone()),
				});
			let info: DispatchInfo = DispatchInfo {
				weight: Weight::from_parts(100, 0),
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let fee = module_transaction_payment::Pallet::<Runtime>::compute_fee(len, &info, 0);
			assert_debug_snapshot!(fee, @"2500000934");

			let surplus_perc = Percent::from_percent(50); // CustomFeeSurplus
			let fee_surplus = surplus_perc.mul_ceil(fee);
			let fee = fee + fee_surplus;
			assert_debug_snapshot!(fee, @"3750001401");

			let alice_native_before = Currencies::free_balance(NATIVE_CURRENCY, &alice_evm_account);
			let alice_erc20_before = Currencies::free_balance(erc20_token, &alice_evm_account);

			// charge fee by default ways
			<EVM as EVMTrait<AccountId>>::set_origin(alice_evm_account.clone());
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&alice_evm_account.clone(),
					call,
					&info,
					len as usize,
				)
			);
			let alice_native_after = Currencies::free_balance(NATIVE_CURRENCY, &alice_evm_account);
			let alice_erc20_after = Currencies::free_balance(erc20_token, &alice_evm_account);

			// charge tx fee and storage fee by native
			assert_debug_snapshot!(alice_native_before - alice_native_after, @"2500000934");
			assert_eq!(alice_erc20_before - alice_erc20_after, 0);

			// cannot charge erc20 fee for the account only hold erc20
			<EVM as EVMTrait<AccountId>>::set_origin(account_with_erc20.clone());
			assert_noop!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&account_with_erc20,
					call,
					&info,
					len as usize,
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);

			let alice_native_before = Currencies::free_balance(NATIVE_CURRENCY, &alice_evm_account);
			let alice_erc20_before = Currencies::free_balance(erc20_token, &alice_evm_account);

			// charge fee by fee pool
			<EVM as EVMTrait<AccountId>>::set_origin(alice_evm_account.clone());
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&alice_evm_account.clone(),
					&with_fee_call,
					&info,
					len as usize,
				)
			);
			let alice_native_after = Currencies::free_balance(NATIVE_CURRENCY, &alice_evm_account);
			let alice_erc20_after = Currencies::free_balance(erc20_token, &alice_evm_account);

			// charge storage by native
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(alice_native_before - alice_native_after, 6400000000);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(alice_native_before - alice_native_after, 6400000000);
			#[cfg(feature = "with-acala-runtime")]
			assert_eq!(alice_native_before - alice_native_after, 19200000000);

			// charge tx fee by native
			#[cfg(feature = "with-mandala-runtime")]
			assert_debug_snapshot!(alice_erc20_before - alice_erc20_after, @"375413056");
			#[cfg(feature = "with-karura-runtime")]
			assert_debug_snapshot!(alice_erc20_before - alice_erc20_after, @"376166142");
			#[cfg(feature = "with-acala-runtime")]
			assert_debug_snapshot!(alice_erc20_before - alice_erc20_after, @"376166142");

			// cannot charge erc20 fee for the account only hold erc20
			<EVM as EVMTrait<AccountId>>::set_origin(account_with_erc20.clone());
			assert_noop!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&account_with_erc20,
					&with_fee_call,
					&info,
					len as usize,
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);
		});
}
