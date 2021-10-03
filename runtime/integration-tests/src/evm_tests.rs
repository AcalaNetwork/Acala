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

use crate::integration_tests::*;

use frame_support::assert_ok;
use module_evm_accounts::EvmAddressMapping;
use module_support::CurrencyIdMapping;
use module_support::{EVMBridge as EVMBridgeT, EVM as EVMTrait};
use primitives::evm::EvmAddress;
use sp_core::bytes::from_hex;
use std::str::FromStr;

#[cfg(feature = "with-karura-runtime")]
use karura_runtime::{EVMBridge, EVM};
#[cfg(feature = "with-mandala-runtime")]
use mandala_runtime::{EVMBridge, EVM};
pub use module_evm_manager::EvmCurrencyIdMapping;

pub fn erc20_address_0() -> EvmAddress {
	EvmAddress::from_str("0000000000000000000000000000000002000000").unwrap()
}

pub fn erc20_address_1() -> EvmAddress {
	EvmAddress::from_str("0000000000000000000000000000000002000001").unwrap()
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000002").unwrap()
}

pub fn lp_erc20() -> CurrencyId {
	CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Erc20(erc20_address_1()))
}

pub fn lp_erc20_evm_address() -> EvmAddress {
	EvmCurrencyIdMapping::<Runtime>::encode_evm_address(lp_erc20()).unwrap()
}

pub fn deploy_erc20_contracts() {
	let code = from_hex(include!("../../../modules/evm-bridge/src/erc20_demo_contract")).unwrap();
	assert_ok!(EVM::create_network_contract(
		Origin::root(),
		code.clone(),
		0,
		2100_000,
		100000
	));

	let event = Event::EVM(module_evm::Event::<Runtime>::Created(erc20_address_0()));
	assert_eq!(System::events().iter().last().unwrap().event, event);

	assert_ok!(EVM::deploy_free(Origin::root(), erc20_address_0()));

	assert_ok!(EVM::create_network_contract(Origin::root(), code, 0, 2100_000, 100000));

	let event = Event::EVM(module_evm::Event::<Runtime>::Created(erc20_address_1()));
	assert_eq!(System::events().iter().last().unwrap().event, event);

	assert_ok!(EVM::deploy_free(Origin::root(), erc20_address_1()));
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

	EVM::create(Origin::signed(account), contract, 0, 1000000000, 100000).map_or_else(|e| Err(e.error), |_| Ok(()))?;

	if let Event::EVM(module_evm::Event::<Runtime>::Created(address)) = System::events().iter().last().unwrap().event {
		Ok(address)
	} else {
		Err("deploy_contract failed".into())
	}
}

#[test]
fn dex_module_works_with_evm_contract() {
	ExtBuilder::default()
		.balances(vec![
			(
				// NetworkContractSource
				MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
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
				Origin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE).encode(), &[][..])
			));

			// CurrencyId::DexShare(Erc20, Erc20)
			assert_ok!(Dex::list_provisioning(
				Origin::root(),
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
				Origin::signed(MockAddressMapping::get_account_id(&alice_evm_addr())),
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
			assert_eq!(
				Currencies::total_issuance(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address_0()),
					DexShare::Erc20(erc20_address_1())
				)),
				0
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Erc20(erc20_address_1())),
					&AccountId::from(ALICE)
				),
				0
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Erc20(erc20_address_1())),
					&MockAddressMapping::get_account_id(&alice_evm_addr())
				),
				0
			);

			// CurrencyId::DexShare(Erc20, Erc20)
			<EVM as EVMTrait<AccountId>>::set_origin(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr()));

			assert_ok!(Dex::add_provision(
				Origin::signed(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				100,
				1000,
			));
			assert_ok!(Dex::end_provisioning(
				Origin::signed(AccountId::from(BOB)),
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

			assert_eq!(
				Currencies::total_issuance(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address_0()),
					DexShare::Erc20(erc20_address_1())
				)),
				220
			);

			assert_ok!(Dex::claim_dex_share(
				Origin::signed(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())),
				EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr()),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
			));
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Erc20(erc20_address_1())),
					&EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())
				),
				220
			);

			assert_ok!(Dex::remove_liquidity(
				Origin::signed(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr())),
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

			assert_eq!(
				Currencies::total_issuance(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address_0()),
					DexShare::Erc20(erc20_address_1())
				)),
				219
			);

			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DexShare(DexShare::Erc20(erc20_address_0()), DexShare::Erc20(erc20_address_1())),
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

			let _alice_address = EvmAccounts::eth_address(&alice_key());
			let bob_address = EvmAccounts::eth_address(&bob_key());

			let contract = deploy_contract(alice()).unwrap();
			System::assert_last_event(Event::EVM(module_evm::Event::Created(contract)));

			assert_ok!(EVM::transfer_maintainer(Origin::signed(alice()), contract, bob_address));
			System::assert_last_event(Event::EVM(module_evm::Event::TransferredMaintainer(
				contract,
				bob_address,
			)));

			// test EvmAccounts Lookup
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 998_963_300_000_000);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(Balances::free_balance(alice()), 996_889_900_000_000);
			assert_eq!(Balances::free_balance(bob()), 1_000 * dollar(NATIVE_CURRENCY));
			let to = EvmAccounts::eth_address(&alice_key());
			assert_ok!(Currencies::transfer(
				Origin::signed(bob()),
				MultiAddress::Address20(to.0),
				NATIVE_CURRENCY,
				10 * dollar(NATIVE_CURRENCY)
			));
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(Balances::free_balance(alice()), 1_008_963_300_000_000);
			#[cfg(feature = "with-karura-runtime")]
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
				// NetworkContractSource
				MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
				NATIVE_CURRENCY,
				(1_000_000_000_000_000_000u128),
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
				Origin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice_key()),
				EvmAccounts::eth_sign(&alice_key(), &AccountId::from(ALICE).encode(), &[][..])
			));
			assert_ok!(Dex::list_provisioning(
				Origin::root(),
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
				Origin::signed(MockAddressMapping::get_account_id(&alice_evm_addr())),
				CurrencyId::Erc20(erc20_address_0()),
				CurrencyId::Erc20(erc20_address_1()),
				100,
				1000,
			));
			assert_ok!(Dex::end_provisioning(
				Origin::signed(AccountId::from(ALICE)),
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
				Origin::signed(MockAddressMapping::get_account_id(&alice_evm_addr())),
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

			let invoke_context = module_support::InvokeContext {
				contract: lp_erc20_evm_address(),
				sender: alice_evm_addr(),
				origin: alice_evm_addr(),
			};

			assert_eq!(
				EVMBridge::name(invoke_context),
				Ok(b"LP long string name, long string name, long string name, long string name, long string name - long string name, long string name, long string name, long string name, long string name"[..32].to_vec())
			);
			assert_eq!(
				EVMBridge::symbol(invoke_context),
				Ok(b"LP_TestToken_TestToken".to_vec())
			);
			assert_eq!(
				EVMBridge::decimals(invoke_context),
				Ok(17)
			);
			assert_eq!(
				EVMBridge::total_supply(invoke_context),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::balance_of(invoke_context, alice_evm_addr()),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::total_supply(invoke_context),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::balance_of(invoke_context, alice_evm_addr()),
				Ok(200)
			);
			assert_eq!(
				EVMBridge::transfer(invoke_context, bob_evm_addr(), 1),
				Ok(())
			);
			assert_eq!(
				EVMBridge::balance_of(invoke_context, alice_evm_addr()),
				Ok(199)
			);
			assert_eq!(
				EVMBridge::balance_of(invoke_context, bob_evm_addr()),
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

			assert_ok!(EVM::create(Origin::signed(alice()), code, 2 * dollar(NATIVE_CURRENCY), 1000000000, 100000));

			let contract = if let Event::EVM(module_evm::Event::Created(address)) = System::events().iter().last().unwrap().event {
				address
			} else {
				panic!("deploy contract failed");
			};

			assert_eq!(Balances::free_balance(EvmAddressMapping::<Runtime>::get_account_id(&contract)), 2 * dollar(NATIVE_CURRENCY));

			#[cfg(all(not(feature = "with-ethereum-compatibility"), feature = "with-mandala-runtime"))]
			assert_eq!(Balances::free_balance(alice()), 1_996_993_800_000_000);
			#[cfg(all(not(feature = "with-ethereum-compatibility"), feature = "with-karura-runtime"))]
			assert_eq!(Balances::free_balance(alice()), 1_994_981_400_000_000);

			#[cfg(feature = "with-ethereum-compatibility")]
			assert_eq!(Balances::free_balance(alice()), 1_998 * dollar(NATIVE_CURRENCY));

			assert_ok!(Currencies::transfer(
				Origin::signed(EvmAddressMapping::<Runtime>::get_account_id(&contract)),
				alice().into(),
				NATIVE_CURRENCY,
				2 * dollar(NATIVE_CURRENCY)
			));

			assert_eq!(Balances::free_balance(EvmAddressMapping::<Runtime>::get_account_id(&contract)), 0);

			#[cfg(all(not(feature = "with-ethereum-compatibility"), feature = "with-mandala-runtime"))]
			assert_eq!(Balances::free_balance(alice()), 1_998_993_800_000_000);
			#[cfg(all(not(feature = "with-ethereum-compatibility"), feature = "with-karura-runtime"))]
			assert_eq!(Balances::free_balance(alice()), 1_996_981_400_000_000);

			#[cfg(feature = "with-ethereum-compatibility")]
			assert_eq!(Balances::free_balance(alice()), 2000 * dollar(NATIVE_CURRENCY));

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
			assert_ok!(EVM::create(Origin::signed(alice()), code, 0, 1000000000, 100000));
			let contract = if let Event::EVM(module_evm::Event::Created(address)) = System::events().iter().last().unwrap().event {
				address
			} else {
				panic!("deploy contract failed");
			};

			let contract_account_id = EvmAddressMapping::<Runtime>::get_account_id(&contract);

			assert_ok!(Currencies::transfer(
				Origin::signed(alice()),
				contract_account_id.clone().into(),
				USD_CURRENCY,
				2 * dollar(USD_CURRENCY)
			));

			assert_eq!(Currencies::free_balance(USD_CURRENCY, &alice()), 998 * dollar(USD_CURRENCY));
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &contract_account_id), 2 * dollar(USD_CURRENCY));
			assert_eq!(EVM::accounts(contract).unwrap().nonce, 1);
			assert_ok!(Currencies::transfer(
				Origin::signed(contract_account_id.clone()),
				alice().into(),
				USD_CURRENCY,
				2 * dollar(USD_CURRENCY)
			));
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &contract_account_id), 0);

			assert_eq!(Currencies::free_balance(USD_CURRENCY, &alice()), 1000 * dollar(USD_CURRENCY));

			// assert the contract account is not purged
			assert!(EVM::accounts(contract).is_some());

			assert_ok!(EVM::call(Origin::signed(alice()), contract.clone(), hex_literal::hex!("41c0e1b5").to_vec(), 0, 1000000000, 100000));

			assert_eq!(System::providers(&contract_account_id), 0);
			assert!(EVM::accounts(contract).is_none());

			// should be gone
			assert!(!System::account_exists(&contract_account_id));
		});
}
