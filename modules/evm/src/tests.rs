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

#![cfg(test)]

use super::*;
use mock::{Event, IdleScheduler, *};

use crate::runner::{
	stack::SubstrateStackState,
	state::{StackExecutor, StackState, StackSubstateMetadata},
};
use frame_support::{assert_noop, assert_ok, dispatch::DispatchErrorWithPostInfo};
use module_support::AddressMapping;
use sp_core::{
	bytes::{from_hex, to_hex},
	H160,
};
use sp_runtime::{traits::BadOrigin, AccountId32};
use std::str::FromStr;

#[test]
fn fail_call_return_ok() {
	new_test_ext().execute_with(|| {
		let mut data = [0u8; 32];
		data[0..4].copy_from_slice(b"evm:");
		let signer: AccountId32 = AccountId32::from(data);

		let origin = Origin::signed(signer);
		assert_ok!(EVM::call(
			origin.clone(),
			contract_a(),
			Vec::new(),
			0,
			1000000,
			0,
			vec![]
		));
		assert_ok!(EVM::call(origin, contract_b(), Vec::new(), 0, 1000000, 0, vec![]));
	});
}

#[test]
fn should_calculate_contract_address() {
	new_test_ext().execute_with(|| {
		let addr = H160::from_str("bec02ff0cbf20042a37d964c33e89f1a2be7f068").unwrap();

		let vicinity = Vicinity {
			gas_price: U256::one(),
			..Default::default()
		};
		let metadata = StackSubstateMetadata::new(1000, 1000, &ACALA_CONFIG);
		let state = SubstrateStackState::<Runtime>::new(&vicinity, metadata);
		let mut executor = StackExecutor::new_with_precompiles(state, &ACALA_CONFIG, &());

		assert_eq!(
			executor.create_address(evm::CreateScheme::Legacy { caller: addr }),
			Ok(H160::from_str("d654cB21c05cb14895baae28159b1107e9DbD6E4").unwrap())
		);

		executor.state_mut().inc_nonce(addr);
		assert_eq!(
			executor.create_address(evm::CreateScheme::Legacy { caller: addr }),
			Ok(H160::from_str("97784910F057B07bFE317b0552AE23eF34644Aed").unwrap())
		);

		executor.state_mut().inc_nonce(addr);
		assert_eq!(
			executor.create_address(evm::CreateScheme::Legacy { caller: addr }),
			Ok(H160::from_str("82155a21E0Ccaee9D4239a582EB2fDAC1D9237c5").unwrap())
		);

		assert_eq!(
			executor.create_address(evm::CreateScheme::Fixed(
				H160::from_str("0x0000000000000000000000000000000000000000").unwrap()
			)),
			Ok(H160::from_str("0x0000000000000000000000000000000000000000").unwrap())
		);
	});
}

#[test]
fn should_create_and_call_contract() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		// publish contract
		let caller = alice();
		let result = <Runtime as Config>::Runner::create(
			caller,
			contract,
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let contract_address = result.value;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(contract_address);

		assert_eq!(contract_address, H160::from_str("5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d").unwrap());

		assert_eq!(Pallet::<Runtime>::account_basic(&caller).nonce, 2.into());

		// multiply(2, 3)
		let multiply = from_hex(
			"0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003"
		).unwrap();

		// call method `multiply`
		let result =  <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			multiply,
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		).unwrap();
		assert_eq!(
			U256::from(result.value.as_slice()),
			6.into(),
		);

		assert_eq!(Pallet::<Runtime>::account_basic(&caller).nonce, 3.into());

		let code_hash = H256::from_str("164981e02df203a0fb32a0af7c2cd1cc7f9df7bb49a4d2b0219307bb68a4b603").unwrap();
		let code_size = 184u32;

		assert_eq!(Accounts::<Runtime>::get(&contract_address), Some(AccountInfo {
			nonce: 1,
			contract_info: Some(ContractInfo {
				code_hash,
				maintainer: alice(),
				published: true
			})
		}));

		assert_eq!(ContractStorageSizes::<Runtime>::get(&contract_address), code_size + NEW_CONTRACT_EXTRA_BYTES);
		assert_eq!(CodeInfos::<Runtime>::get(&code_hash), Some(CodeInfo {
			code_size,
			ref_count: 1,
		}));
		assert!(Codes::<Runtime>::contains_key(&code_hash));
	});
}

#[test]
fn create_reverts_with_message() {
	// pragma solidity ^0.5.0;
	//
	// contract Foo {
	//     constructor() public {
	// 		require(false, "error message");
	// 	}
	// }
	let contract = from_hex(
		"0x6080604052348015600f57600080fd5b5060006083576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040180806020018281038252600d8152602001807f6572726f72206d6573736167650000000000000000000000000000000000000081525060200191505060405180910390fd5b603e8060906000396000f3fe6080604052600080fdfea265627a7a723158204741083d83bf4e3ee8099dd0b3471c81061237c2e8eccfcb513dfa4c04634b5b64736f6c63430005110032"
	).unwrap();
	new_test_ext().execute_with(|| {
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract,
			0,
			12_000_000,
			12_000_000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert_eq!(
			result.value,
			H160::from_str("0x5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d").unwrap()
		);
	});
}

#[test]
fn call_reverts_with_message() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	// 	function foo() public pure {
	// 		require(false, "error message");
	// 	}
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060df8061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c298557814602d575b600080fd5b60336035565b005b600060a8576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040180806020018281038252600d8152602001807f6572726f72206d6573736167650000000000000000000000000000000000000081525060200191505060405180910390fd5b56fea265627a7a7231582066b3ee33bedba8a318d0d66610145030fdc0f982b11f5160d366e15e4d8ba2ef64736f6c63430005110032"
	).unwrap();
	let caller = alice();

	new_test_ext().execute_with(|| {
		// publish contract
		let result = <Runtime as Config>::Runner::create(
			caller,
			contract,
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		).unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let alice_balance = INITIAL_BALANCE - 323 * EVM::get_storage_deposit_per_byte();

		assert_eq!(balance(alice()), alice_balance);

		let contract_address = result.value;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(contract_address);

		// call method `foo`
		let foo = from_hex("0xc2985578").unwrap();
		let result = <Runtime as Config>::Runner::call(
			caller,
			caller,
			contract_address,
			foo,
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		).unwrap();

		assert_eq!(balance(alice()), alice_balance);
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert_eq!(
			to_hex(&result.value, true),
			"0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676500000000000000000000000000000000000000"
		);

		let message  = String::from_utf8_lossy(&result.value);
		assert!(message.contains("error message"));

		assert_eq!(Pallet::<Runtime>::account_basic(&caller).nonce, 3.into());
	});
}

#[test]
fn should_publish_payable_contract() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	// 	 uint value;
	// 	 constructor(uint a) public payable {
	// 		value = a;
	// 	 }
	//
	//   function getValue() public payable returns (uint) {
	// 	     return value;
	// 	 }
	// }
	let mut contract = from_hex(
		"0x60806040526040516100c73803806100c783398181016040526020811015602557600080fd5b81019080805190602001909291905050508060008190555050607b8061004c6000396000f3fe608060405260043610601c5760003560e01c806320965255146021575b600080fd5b6027603d565b6040518082815260200191505060405180910390f35b6000805490509056fea265627a7a72315820b832564a9db725638dcef03d07bfbdd2dc818020ea359630317e2126e95c314964736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		let amount = 1000u128;

		let stored_value: Vec<u8> =
			from_hex("0x000000000000000000000000000000000000000000000000000000000000007b").unwrap();
		contract.append(&mut stored_value.clone());

		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract.clone(),
			convert_decimals_to_evm(amount),
			1000000,
			100000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		let contract_address = result.value;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(contract_address);

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 287);

		let alice_balance = INITIAL_BALANCE - amount - 287 * EVM::get_storage_deposit_per_byte();
		assert_eq!(balance(alice()), alice_balance);
		assert_eq!(balance(contract_address), amount);

		// call getValue()
		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			from_hex("0x20965255").unwrap(),
			convert_decimals_to_evm(amount),
			100000,
			100000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.value, stored_value);
		assert_eq!(result.used_storage, 0);

		assert_eq!(balance(alice()), alice_balance - amount);
		assert_eq!(balance(contract_address), 2 * amount);

		assert_eq!(
			AccountStorages::<Runtime>::iter_prefix(&contract_address).collect::<Vec<_>>(),
			vec![(
				H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000").unwrap(),
				H256::from_slice(stored_value.as_slice())
			)]
		);
	});
}

#[test]
fn should_transfer_from_contract() {
	// pragma solidity ^0.5.16;
	//
	// contract SendEther {
	//     function sendViaTransfer(address payable _to) public payable {
	//         // This function is no longer recommended for sending Ether.
	//         _to.transfer(msg.value);
	//     }
	//
	//     function sendOneEthViaTransfer(address payable _to) public {
	//         // This function is no longer recommended for sending Ether.
	//         _to.transfer(1 ether);
	//     }
	//
	//     function balanceOf(address _to) public view returns (uint256) {
	//         return _to.balance;
	//     }
	//
	//     function sendViaSend(address payable _to) public payable {
	//         // Send returns a boolean value indicating success or failure.
	//         // This function is not recommended for sending Ether.
	//         bool sent = _to.send(msg.value);
	//         require(sent, "Failed to send Ether");
	//     }
	//
	//     function sendViaCall(address payable _to) public payable {
	//         // Call returns a boolean value indicating success or failure.
	//         // This is the current recommended method to use.
	//         (bool sent, bytes memory data) = _to.call.value(msg.value)("");
	//         require(sent, "Failed to send Ether");
	//     }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5061044a806100206000396000f3fe60806040526004361061004a5760003560e01c8063636e082b1461004f57806370a082311461009357806372005fce146100f857806374be48061461013c578063830c29ae14610180575b600080fd5b6100916004803603602081101561006557600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff1690602001909291905050506101c4565b005b34801561009f57600080fd5b506100e2600480360360208110156100b657600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919050505061020e565b6040518082815260200191505060405180910390f35b61013a6004803603602081101561010e57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919050505061022f565b005b61017e6004803603602081101561015257600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050610281565b005b6101c26004803603602081101561019657600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050610331565b005b8073ffffffffffffffffffffffffffffffffffffffff166108fc349081150290604051600060405180830381858888f1935050505015801561020a573d6000803e3d6000fd5b5050565b60008173ffffffffffffffffffffffffffffffffffffffff16319050919050565b8073ffffffffffffffffffffffffffffffffffffffff166108fc670de0b6b3a76400009081150290604051600060405180830381858888f1935050505015801561027d573d6000803e3d6000fd5b5050565b60008173ffffffffffffffffffffffffffffffffffffffff166108fc349081150290604051600060405180830381858888f1935050505090508061032d576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260148152602001807f4661696c656420746f2073656e6420457468657200000000000000000000000081525060200191505060405180910390fd5b5050565b600060608273ffffffffffffffffffffffffffffffffffffffff163460405180600001905060006040518083038185875af1925050503d8060008114610393576040519150601f19603f3d011682016040523d82523d6000602084013e610398565b606091505b509150915081610410576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260148152602001807f4661696c656420746f2073656e6420457468657200000000000000000000000081525060200191505060405180910390fd5b50505056fea265627a7a7231582021fdf580e8b027bad2c5950c8a3292801da3f6f119a9dddcf170592ed45c85f264736f6c63430005100032"
	).unwrap();
	new_test_ext().execute_with(|| {
		let amount = 1000u128;

		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract,
			0,
			10000000,
			10000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.expect("create shouldn't fail");
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 1198);

		let alice_balance = INITIAL_BALANCE - 1198 * EVM::get_storage_deposit_per_byte();
		assert_eq!(balance(alice()), alice_balance);
		assert_eq!(
			eth_balance(alice()),
			U256::from(convert_decimals_to_evm(balance(alice())))
		);

		let contract_address = result.value;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(contract_address);

		// send via transfer
		let mut via_transfer = from_hex("0x636e082b").unwrap();
		via_transfer.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			via_transfer,
			convert_decimals_to_evm(amount),
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(alice()), alice_balance - amount);
		assert_eq!(
			eth_balance(alice()),
			U256::from(convert_decimals_to_evm(balance(alice())))
		);
		assert_eq!(balance(charlie()), amount);
		assert_eq!(
			eth_balance(charlie()),
			U256::from(convert_decimals_to_evm(balance(charlie())))
		);

		// send via send
		let mut via_send = from_hex("0x74be4806").unwrap();
		via_send.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			via_send,
			convert_decimals_to_evm(amount),
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(charlie()), 2 * amount);
		assert_eq!(
			eth_balance(charlie()),
			U256::from(convert_decimals_to_evm(balance(charlie())))
		);
		assert_eq!(balance(alice()), alice_balance - 2 * amount);
		assert_eq!(
			eth_balance(alice()),
			U256::from(convert_decimals_to_evm(balance(alice())))
		);

		// send via call
		let mut via_call = from_hex("0x830c29ae").unwrap();
		via_call.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			via_call,
			convert_decimals_to_evm(amount),
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(charlie()), 3 * amount);
		assert_eq!(
			eth_balance(charlie()),
			U256::from(convert_decimals_to_evm(balance(charlie())))
		);
		assert_eq!(balance(alice()), alice_balance - 3 * amount);
		assert_eq!(
			eth_balance(alice()),
			U256::from(convert_decimals_to_evm(balance(alice())))
		);

		// send 1 eth via transfer
		let dollar_aca = 10u128.pow(12);
		let mut one_eth_via_transfer = from_hex("0x72005fce").unwrap();
		one_eth_via_transfer.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			one_eth_via_transfer,
			convert_decimals_to_evm(dollar_aca), // 1 ACA
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(charlie()), 3 * amount + dollar_aca);
		assert_eq!(
			eth_balance(charlie()),
			U256::from(convert_decimals_to_evm(balance(charlie())))
		);
		assert_eq!(balance(alice()), alice_balance - 3 * amount - dollar_aca);
		assert_eq!(
			eth_balance(alice()),
			U256::from(convert_decimals_to_evm(balance(alice())))
		);

		// balanceOf
		let mut one_eth_via_transfer = from_hex("0x70a08231").unwrap();
		one_eth_via_transfer.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			one_eth_via_transfer,
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(
			U256::from(result.value.as_slice()),
			U256::from(convert_decimals_to_evm(balance(charlie())))
		);
	})
}

#[test]
fn contract_should_publish_contracts() {
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
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a40029"
	).unwrap();
	new_test_ext().execute_with(|| {
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract.clone(),
			0,
			1000000000,
			1000000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 467);

		let alice_balance = INITIAL_BALANCE - 467 * EVM::get_storage_deposit_per_byte();

		assert_eq!(balance(alice()), alice_balance);
		let factory_contract_address = result.value;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(factory_contract_address);

		assert_eq!(balance(factory_contract_address), 0);
		assert_eq!(
			reserved_balance(factory_contract_address),
			467 * EVM::get_storage_deposit_per_byte()
		);

		// Factory.createContract
		let amount = 1000u128;
		let create_contract = from_hex("0x412a5a6d").unwrap();
		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			convert_decimals_to_evm(amount),
			1000000000,
			1000000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 281);

		assert_eq!(
			balance(alice()),
			alice_balance - amount - 281 * EVM::get_storage_deposit_per_byte()
		);
		assert_eq!(balance(factory_contract_address), amount);
		assert_eq!(
			reserved_balance(factory_contract_address),
			(467 + 128) * EVM::get_storage_deposit_per_byte()
		);
		let contract_address = H160::from_str("7b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap();
		assert_eq!(balance(contract_address), 0);
		assert_eq!(
			reserved_balance(contract_address),
			153 * EVM::get_storage_deposit_per_byte()
		);
	});
}

#[test]
fn contract_should_publish_contracts_without_payable() {
	// pragma solidity ^0.5.0;
	//
	// contract Factory {
	//     Contract[] newContracts;
	//
	//     function createContract () public {
	//         Contract newContract = new Contract();
	//         newContracts.push(newContract);
	//     }
	// }
	//
	// contract Contract {}
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5061016c806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063412a5a6d14610030575b600080fd5b61003861003a565b005b6000604051610048906100d0565b604051809103906000f080158015610064573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b605b806100dd8339019056fe6080604052348015600f57600080fd5b50603e80601d6000396000f3fe6080604052600080fdfea265627a7a7231582094976cee5af14bf59c4bae67c79c12eb15de19bc18ad6038f3ee0898273c9c0564736f6c63430005110032a265627a7a72315820e19ae28dbf01eae11c526295a1ac533ea341c74d5724efe43171f6010fc98b3964736f6c63430005110032"
	).unwrap();
	new_test_ext().execute_with(|| {
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract.clone(),
			0,
			1000000000,
			1000000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let alice_balance = INITIAL_BALANCE - 464 * EVM::get_storage_deposit_per_byte();

		assert_eq!(balance(alice()), alice_balance);
		let factory_contract_address = result.value;
		assert_eq!(balance(factory_contract_address), 0);
		assert_eq!(reserved_balance(factory_contract_address), 4640);

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(factory_contract_address);

		// Factory.createContract
		let create_contract = from_hex("0x412a5a6d").unwrap();
		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			0,
			1000000000,
			1000000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 290);
		assert_eq!(
			balance(alice()),
			alice_balance - (result.used_storage as u128 * EVM::get_storage_deposit_per_byte())
		);
		assert_eq!(balance(factory_contract_address), 0);
		assert_eq!(
			reserved_balance(factory_contract_address),
			(464 + 128) * EVM::get_storage_deposit_per_byte()
		);
	});
}

#[test]
fn publish_factory() {
	// pragma solidity ^0.5.0;
	//
	// contract Factory {
	//     Contract c;
	//     constructor() public {
	//         c = new Contract();
	//         c.foo();
	//     }
	// }
	//
	// contract Contract {
	//     function foo() public pure returns (uint) {
	//         return 123;
	//     }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060405161001d90610121565b604051809103906000f080158015610039573d6000803e3d6000fd5b506000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1663c29855786040518163ffffffff1660e01b815260040160206040518083038186803b1580156100e057600080fd5b505afa1580156100f4573d6000803e3d6000fd5b505050506040513d602081101561010a57600080fd5b81019080805190602001909291905050505061012d565b60a58061017983390190565b603e8061013b6000396000f3fe6080604052600080fdfea265627a7a7231582064177030ee644a03aaf8d65027df9e0331c8bc4b161de25bfb8aa3142848e0f864736f6c634300051100326080604052348015600f57600080fd5b5060878061001e6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c298557814602d575b600080fd5b60336049565b6040518082815260200191505060405180910390f35b6000607b90509056fea265627a7a7231582031e5a4abae00962cfe9875df1b5b0d3ce6624e220cb8c714a948794fcddb6b4f64736f6c63430005110032"
	).unwrap();
	new_test_ext().execute_with(|| {
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract,
			0,
			2_000_000,
			5000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_gas.as_u64(), 155_879u64);
		assert_eq!(result.used_storage, 461);
		assert_eq!(
			balance(alice()),
			INITIAL_BALANCE - (result.used_storage as u128 * EVM::get_storage_deposit_per_byte())
		);
	});
}

#[test]
fn create_nft_contract_works() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		// publish contract
		assert_ok!(EVM::create_nft_contract(
			Origin::signed(NetworkContractAccount::get()),
			contract,
			0,
			1000000,
			1000000,
			vec![],
		));

		assert_eq!(
			Pallet::<Runtime>::account_basic(&NetworkContractSource::get()).nonce,
			2.into()
		);
		System::assert_last_event(Event::EVM(crate::Event::Created {
			from: NetworkContractSource::get(),
			contract: MIRRORED_TOKENS_ADDRESS_START | H160::from_low_u64_be(MIRRORED_NFT_ADDRESS_START),
			logs: vec![],
			used_gas: 93183,
			used_storage: 284,
		}));
		assert_eq!(EVM::network_contract_index(), MIRRORED_NFT_ADDRESS_START + 1);
	});
}

#[test]
fn create_nft_contract_fails_if_non_network_contract_origin() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		assert_noop!(
			EVM::create_nft_contract(
				Origin::signed(AccountId32::from([1u8; 32])),
				contract,
				0,
				1000000,
				1000000,
				vec![],
			),
			BadOrigin
		);
	});
}

#[test]
fn create_predeploy_contract_works() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		let addr = H160::from_str("1111111111111111111111111111111111111111").unwrap();

		assert_eq!(Pallet::<Runtime>::is_account_empty(&addr), true);

		// deploy contract
		assert_ok!(EVM::create_predeploy_contract(
			Origin::signed(NetworkContractAccount::get()),
			addr,
			contract.clone(),
			0,
			1000000,
			1000000,
			vec![],
		));

		assert_eq!(Pallet::<Runtime>::is_account_empty(&addr), false);

		System::assert_has_event(Event::EVM(crate::Event::Created {
			from: NetworkContractSource::get(),
			contract: addr,
			logs: vec![],
			used_gas: 93183,
			used_storage: 284,
		}));

		System::assert_last_event(Event::EVM(crate::Event::ContractPublished { contract: addr }));

		assert_noop!(
			EVM::create_predeploy_contract(
				Origin::signed(NetworkContractAccount::get()),
				addr,
				vec![],
				0,
				1000000,
				1000000,
				vec![],
			),
			Error::<Runtime>::ContractAlreadyExisted
		);

		// deploy empty contract
		let token_addr = H160::from_str("2222222222222222222222222222222222222222").unwrap();
		assert_noop!(
			EVM::create_predeploy_contract(
				Origin::signed(NetworkContractAccount::get()),
				token_addr,
				vec![],
				0,
				1000000,
				1000000,
				vec![],
			),
			Error::<Runtime>::ContractNotFound
		);

		assert_eq!(CodeInfos::<Runtime>::get(&EVM::code_hash_at_address(&token_addr)), None);
	});
}

#[test]
fn should_transfer_maintainer() {
	// pragma solidity ^0.5.0;
	//
	// contract Factory {
	//     Contract c;
	//     constructor() public {
	//         c = new Contract();
	//         c.foo();
	//     }
	// }
	//
	// contract Contract {
	//     function foo() public pure returns (uint) {
	//         return 123;
	//     }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060405161001d90610121565b604051809103906000f080158015610039573d6000803e3d6000fd5b506000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1663c29855786040518163ffffffff1660e01b815260040160206040518083038186803b1580156100e057600080fd5b505afa1580156100f4573d6000803e3d6000fd5b505050506040513d602081101561010a57600080fd5b81019080805190602001909291905050505061012d565b60a58061017983390190565b603e8061013b6000396000f3fe6080604052600080fdfea265627a7a7231582064177030ee644a03aaf8d65027df9e0331c8bc4b161de25bfb8aa3142848e0f864736f6c634300051100326080604052348015600f57600080fd5b5060878061001e6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c298557814602d575b600080fd5b60336049565b6040518082815260200191505060405180910390f35b6000607b90509056fea265627a7a7231582031e5a4abae00962cfe9875df1b5b0d3ce6624e220cb8c714a948794fcddb6b4f64736f6c63430005110032"
	).unwrap();
	new_test_ext().execute_with(|| {
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract,
			0,
			12_000_000,
			12_000_000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 461);
		let alice_balance = INITIAL_BALANCE - 461 * EVM::get_storage_deposit_per_byte();
		let contract_address = result.value;

		assert_eq!(balance(alice()), alice_balance);

		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Runtime as Config>::AddressMapping::get_account_id(&bob());
		assert_eq!(balance(bob()), INITIAL_BALANCE);
		// transfer_maintainer
		assert_ok!(EVM::transfer_maintainer(
			Origin::signed(alice_account_id.clone()),
			contract_address,
			bob()
		));
		System::assert_last_event(Event::EVM(crate::Event::TransferredMaintainer {
			contract: contract_address,
			new_maintainer: bob(),
		}));
		assert_eq!(balance(bob()), INITIAL_BALANCE);

		assert_noop!(
			EVM::transfer_maintainer(Origin::signed(bob_account_id), H160::default(), alice()),
			Error::<Runtime>::ContractNotFound
		);

		assert_noop!(
			EVM::transfer_maintainer(Origin::signed(alice_account_id), contract_address, bob()),
			Error::<Runtime>::NoPermission
		);
		assert_eq!(balance(alice()), alice_balance);
	});
}

#[test]
fn should_publish() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Runtime as Config>::AddressMapping::get_account_id(&bob());

		// contract not created yet
		assert_noop!(EVM::publish_contract(Origin::signed(alice_account_id.clone()), H160::default()), Error::<Runtime>::ContractNotFound);

		// if the contract not exists, evm will return ExitSucceed::Stopped.
		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			EvmAddress::default(),
			vec![],
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 0);

		// create contract
		let result = <Runtime as Config>::Runner::create(alice(), contract, 0, 21_000_000, 21_000_000, vec![],<Runtime as Config>::config()).unwrap();
		let contract_address = result.value;

		assert_eq!(result.used_storage, 284);
		let alice_balance = INITIAL_BALANCE - 284 * EVM::get_storage_deposit_per_byte();

		assert_eq!(balance(alice()), alice_balance);

		// multiply(2, 3)
		let multiply = from_hex(
			"0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003"
		).unwrap();

		// contract maintainer can call
		assert_ok!(<Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		));

		// call method `multiply` will fail, not published yet
		assert_eq!(EVM::call(
			Origin::signed(bob_account_id.clone()),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
			vec![],
		), Ok(PostDispatchInfo { actual_weight: None, pays_fee: Pays::Yes }));
		System::assert_last_event(Event::EVM(crate::Event::ExecutedFailed {
			from: bob(),
			contract: contract_address,
			exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(Error::<Runtime>::NoPermission).into())),
			output: vec![],
			logs: vec![],
			used_gas: 1000000,
			used_storage: 0,
		}));

		// developer can call the unpublished contract
		assert_ok!(EVM::enable_contract_development(Origin::signed(bob_account_id.clone())));
		assert_ok!(<Runtime as Config>::Runner::call(
			bob(),
			bob(),
			contract_address,
			vec![],
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		));

		// not maintainer
		assert_noop!(EVM::publish_contract(Origin::signed(bob_account_id), contract_address), Error::<Runtime>::NoPermission);

		assert_ok!(EVM::publish_contract(Origin::signed(alice_account_id.clone()), contract_address));
		let code_size = Accounts::<Runtime>::get(contract_address).map_or(0, |account_info| -> u32 {
			account_info.contract_info.map_or(0, |contract_info| CodeInfos::<Runtime>::get(contract_info.code_hash).map_or(0, |code_info| code_info.code_size))
		});
		assert_eq!(balance(alice()), INITIAL_BALANCE - PUBLICATION_FEE - ((NEW_CONTRACT_EXTRA_BYTES + code_size) as u128* EVM::get_storage_deposit_per_byte()));
		assert_eq!(Balances::free_balance(TreasuryAccount::get()), INITIAL_BALANCE + PUBLICATION_FEE);

		// call method `multiply` will work
		assert_ok!(<Runtime as Config>::Runner::call(
			alice(),
			alice(),
			contract_address,
			multiply,
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		));

		// contract already published
		assert_noop!(EVM::publish_contract(Origin::signed(alice_account_id), contract_address), Error::<Runtime>::ContractAlreadyPublished);
	});
}

#[test]
fn should_publish_free() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		// contract not created yet
		assert_noop!(EVM::publish_free(Origin::signed(CouncilAccount::get()), H160::default()), Error::<Runtime>::ContractNotFound);

		// create contract
		let result = <Runtime as Config>::Runner::create(alice(), contract, 0, 21_000_000, 21_000_000, vec![], <Runtime as Config>::config()).unwrap();
		let contract_address = result.value;

		// multiply(2, 3)
		let multiply = from_hex(
			"0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003"
		).unwrap();

		// call method `multiply` will fail, not published yet
		let bob_account_id = <Runtime as Config>::AddressMapping::get_account_id(&bob());
		assert_eq!(EVM::call(
			Origin::signed(bob_account_id),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
			vec![],
		), Ok(PostDispatchInfo { actual_weight: None, pays_fee: Pays::Yes }));
		System::assert_last_event(Event::EVM(crate::Event::ExecutedFailed {
			from: bob(),
			contract: contract_address,
			exit_reason: ExitReason::Error(ExitError::Other(Into::<&str>::into(Error::<Runtime>::NoPermission).into())),
			output: vec![],
			logs: vec![],
			used_gas: 1000000,
			used_storage: 0,
		}));

		assert_ok!(EVM::publish_free(Origin::signed(CouncilAccount::get()), contract_address));

		// call method `multiply`
		assert_ok!(<Runtime as Config>::Runner::call(
			bob(),
			alice(),
			contract_address,
			multiply,
			0,
			1000000,
			1000000,
			vec![],
			<Runtime as Config>::config(),
		));

		// contract already published
		assert_noop!(EVM::publish_free(Origin::signed(CouncilAccount::get()), contract_address), Error::<Runtime>::ContractAlreadyPublished);
	});
}

#[test]
fn should_enable_contract_development() {
	new_test_ext().execute_with(|| {
		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());
		assert_eq!(reserved_balance(alice()), 0);
		assert_ok!(EVM::enable_contract_development(Origin::signed(alice_account_id)));
		assert_eq!(reserved_balance(alice()), DEVELOPER_DEPOSIT);
		assert_eq!(balance(alice()), INITIAL_BALANCE - DEVELOPER_DEPOSIT);
	});
}

#[test]
fn should_disable_contract_development() {
	new_test_ext().execute_with(|| {
		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());

		// contract development is not enabled yet
		assert_noop!(
			EVM::disable_contract_development(Origin::signed(alice_account_id.clone())),
			Error::<Runtime>::ContractDevelopmentNotEnabled
		);
		assert_eq!(balance(alice()), INITIAL_BALANCE);

		// enable contract development
		assert_eq!(reserved_balance(alice()), 0);
		assert_ok!(EVM::enable_contract_development(Origin::signed(
			alice_account_id.clone()
		)));
		assert_eq!(reserved_balance(alice()), DEVELOPER_DEPOSIT);

		// deposit reserved
		assert_eq!(balance(alice()), INITIAL_BALANCE - DEVELOPER_DEPOSIT);

		// disable contract development
		assert_ok!(EVM::disable_contract_development(Origin::signed(
			alice_account_id.clone()
		)));
		// deposit unreserved
		assert_eq!(balance(alice()), INITIAL_BALANCE);

		// contract development already disabled
		assert_noop!(
			EVM::disable_contract_development(Origin::signed(alice_account_id)),
			Error::<Runtime>::ContractDevelopmentNotEnabled
		);
	});
}

#[test]
fn should_set_code() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	let contract_err = from_hex(
		"0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Runtime as Config>::AddressMapping::get_account_id(&bob());

		// create contract
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract.clone(),
			0,
			21_000_000,
			21_000_000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		let contract_address = result.value;
		assert_eq!(result.used_storage, 284);
		let alice_balance = INITIAL_BALANCE - 284 * EVM::get_storage_deposit_per_byte();

		assert_eq!(balance(alice()), alice_balance);
		assert_eq!(reserved_balance(contract_address), 2840);

		let code_hash = H256::from_str("164981e02df203a0fb32a0af7c2cd1cc7f9df7bb49a4d2b0219307bb68a4b603").unwrap();
		assert_eq!(
			Accounts::<Runtime>::get(&contract_address),
			Some(AccountInfo {
				nonce: 1,
				contract_info: Some(ContractInfo {
					code_hash,
					maintainer: alice(),
					published: false
				})
			})
		);
		assert_eq!(
			CodeInfos::<Runtime>::get(&code_hash),
			Some(CodeInfo {
				code_size: 184,
				ref_count: 1,
			})
		);

		assert_noop!(
			EVM::set_code(Origin::signed(bob_account_id), contract_address, contract.clone()),
			Error::<Runtime>::NoPermission
		);
		assert_ok!(EVM::set_code(
			Origin::signed(alice_account_id.clone()),
			contract_address,
			contract.clone()
		));
		assert_ok!(EVM::set_code(Origin::root(), contract_address, contract));

		assert_eq!(reserved_balance(contract_address), 4150);

		let new_code_hash = H256::from_str("9061d510f6235de4eae304e1a2a2ae22e1610ba893c018b7fabc1f1635f49877").unwrap();
		assert_eq!(
			Accounts::<Runtime>::get(&contract_address),
			Some(AccountInfo {
				nonce: 1,
				contract_info: Some(ContractInfo {
					code_hash: new_code_hash,
					maintainer: alice(),
					published: false
				})
			})
		);
		assert_eq!(CodeInfos::<Runtime>::get(&code_hash), None);
		assert_eq!(
			CodeInfos::<Runtime>::get(&new_code_hash),
			Some(CodeInfo {
				code_size: 215,
				ref_count: 1,
			})
		);
		assert_eq!(Codes::<Runtime>::contains_key(&code_hash), false);
		assert_eq!(Codes::<Runtime>::contains_key(&new_code_hash), true);

		assert_ok!(EVM::set_code(Origin::root(), contract_address, vec![]));
		let new_code_hash = H256::from_str("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470").unwrap();
		assert_eq!(
			Accounts::<Runtime>::get(&contract_address),
			Some(AccountInfo {
				nonce: 1,
				contract_info: Some(ContractInfo {
					code_hash: new_code_hash,
					maintainer: alice(),
					published: false
				})
			})
		);
		assert_eq!(
			CodeInfos::<Runtime>::get(&new_code_hash),
			Some(CodeInfo {
				code_size: 0,
				ref_count: 1,
			})
		);
		assert_eq!(reserved_balance(contract_address), 3000);

		assert_noop!(
			EVM::set_code(
				Origin::signed(alice_account_id.clone()),
				contract_address,
				[8u8; (MaxCodeSize::get() + 1) as usize].to_vec(),
			),
			Error::<Runtime>::ContractExceedsMaxCodeSize
		);

		assert_ok!(EVM::publish_free(
			Origin::signed(CouncilAccount::get()),
			contract_address
		));

		assert_noop!(
			EVM::set_code(Origin::signed(alice_account_id), contract_address, contract_err),
			Error::<Runtime>::ContractAlreadyPublished
		);
	});
}

#[test]
fn should_selfdestruct() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	// 	 uint value;
	// 	 constructor(uint a) public payable {
	// 		value = a;
	// 	 }
	//
	//   function getValue() public payable returns (uint) {
	// 	     return value;
	// 	 }
	// }
	let mut contract = from_hex(
		"0x60806040526040516100c73803806100c783398181016040526020811015602557600080fd5b81019080805190602001909291905050508060008190555050607b8061004c6000396000f3fe608060405260043610601c5760003560e01c806320965255146021575b600080fd5b6027603d565b6040518082815260200191505060405180910390f35b6000805490509056fea265627a7a72315820b832564a9db725638dcef03d07bfbdd2dc818020ea359630317e2126e95c314964736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Runtime as Config>::AddressMapping::get_account_id(&bob());

		let amount = 1000u128;

		let mut stored_value: Vec<u8> =
			from_hex("0x000000000000000000000000000000000000000000000000000000000000007b").unwrap();
		contract.append(&mut stored_value);

		// create contract
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract,
			convert_decimals_to_evm(amount),
			1000000,
			100000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		let contract_address = result.value;
		assert_eq!(result.used_storage, 287);
		let alice_balance = INITIAL_BALANCE - 287 * EVM::get_storage_deposit_per_byte() - amount;

		assert_eq!(balance(alice()), alice_balance);

		let code_hash = H256::from_str("21fe816097a50d298f819bc6d40cff473c43c87d99bcd7d3c3b2b85417f66f5a").unwrap();
		let code_size = 123u32;

		assert_eq!(
			ContractStorageSizes::<Runtime>::get(&contract_address),
			code_size + NEW_CONTRACT_EXTRA_BYTES + STORAGE_SIZE
		);
		assert_eq!(
			CodeInfos::<Runtime>::get(&code_hash),
			Some(CodeInfo {
				code_size,
				ref_count: 1,
			})
		);
		assert!(Codes::<Runtime>::contains_key(&code_hash));

		assert_noop!(
			EVM::selfdestruct(Origin::signed(bob_account_id), contract_address),
			Error::<Runtime>::NoPermission
		);
		let contract_account_id = <Runtime as Config>::AddressMapping::get_account_id(&contract_address);
		assert_eq!(System::providers(&contract_account_id), 2);
		assert_ok!(EVM::selfdestruct(Origin::signed(alice_account_id), contract_address));

		assert_eq!(System::providers(&contract_account_id), 1);
		assert!(System::account_exists(&contract_account_id));
		assert!(Accounts::<Runtime>::contains_key(&contract_address));
		assert!(!ContractStorageSizes::<Runtime>::contains_key(&contract_address));
		assert_eq!(AccountStorages::<Runtime>::iter_prefix(&contract_address).count(), 1);
		assert!(!CodeInfos::<Runtime>::contains_key(&code_hash));
		assert!(!Codes::<Runtime>::contains_key(&code_hash));

		assert_eq!(balance(alice()), alice_balance);
		assert_eq!(balance(contract_address), 1000);

		let reserved_amount = 287 * EVM::get_storage_deposit_per_byte();
		assert_eq!(reserved_balance(contract_address), reserved_amount);

		// can't publish at the same address
		assert_noop!(
			EVM::create_predeploy_contract(
				Origin::signed(NetworkContractAccount::get()),
				contract_address,
				vec![],
				0,
				1000000,
				1000000,
				vec![],
			),
			DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: None,
					pays_fee: Pays::Yes,
				},
				error: Error::<Runtime>::ContractAlreadyExisted.into()
			}
		);

		IdleScheduler::on_idle(0, 1_000_000_000_000);

		// refund storage deposit
		assert_eq!(balance(alice()), alice_balance + amount + reserved_amount);
		assert_eq!(balance(contract_address), 0);
		assert_eq!(reserved_balance(contract_address), 0);

		assert_eq!(System::providers(&contract_account_id), 0);
		assert!(!System::account_exists(&contract_account_id));
		assert!(Accounts::<Runtime>::contains_key(&contract_address));
		assert_eq!(AccountStorages::<Runtime>::iter_prefix(&contract_address).count(), 0);
	});
}

#[test]
fn storage_limit_should_work() {
	// pragma solidity ^0.5.0;

	// contract Factory {
	// 	Contract[] newContracts;

	// 	function createContract (uint num) public payable {
	// 		for(uint i = 0; i < num; i++) {
	// 			Contract newContract = new Contract();
	// 			newContracts.push(newContract);
	// 		}
	// 	}
	// }

	// contract Contract {}
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b506101a0806100206000396000f3fe60806040526004361061001e5760003560e01c80639db8d7d514610023575b600080fd5b61004f6004803603602081101561003957600080fd5b8101908080359060200190929190505050610051565b005b60008090505b8181101561010057600060405161006d90610104565b604051809103906000f080158015610089573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff16021790555050508080600101915050610057565b5050565b605b806101118339019056fe6080604052348015600f57600080fd5b50603e80601d6000396000f3fe6080604052600080fdfea265627a7a7231582035666e9471716d6d05ed9f0c1ab13d0371f49d536270f905bff06cd98212dcb064736f6c63430005110032a265627a7a723158203b6aaf6588bc3e6a35986612a62f715255430eab09ffb24401e5f18eb58a05d564736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract.clone(),
			0,
			200_000,
			1000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 516);
		let alice_balance = INITIAL_BALANCE - 516 * EVM::get_storage_deposit_per_byte();
		assert_eq!(balance(alice()), alice_balance);

		let factory_contract_address = result.value;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(factory_contract_address);

		assert_eq!(balance(factory_contract_address), 0);
		assert_eq!(
			reserved_balance(factory_contract_address),
			516 * EVM::get_storage_deposit_per_byte()
		);

		// Factory.createContract(1)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());
		assert_eq!(
			EVM::call(
				Origin::signed(alice_account_id.clone()),
				factory_contract_address,
				create_contract,
				amount,
				1000000000,
				0,
				vec![],
			),
			Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes
			})
		);
		System::assert_last_event(Event::EVM(crate::Event::ExecutedFailed {
			from: alice(),
			contract: factory_contract_address,
			exit_reason: ExitReason::Error(ExitError::Other(
				Into::<&str>::into(Error::<Runtime>::OutOfStorage).into(),
			)),
			output: vec![],
			logs: vec![],
			used_gas: 1000000000,
			used_storage: 0,
		}));

		// Factory.createContract(1)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			amount,
			1000000000,
			1000000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));

		// code_size + array_update(2 items) + extra_size = 290, array_length is already set
		let expected_used_storage = 62 + 2 * 64 + 100;
		assert_eq!(expected_used_storage, 290);
		assert_eq!(result.used_storage, expected_used_storage);

		// Factory.createContract(2)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000002").unwrap();
		assert_eq!(
			EVM::call(
				Origin::signed(alice_account_id),
				factory_contract_address,
				create_contract,
				amount,
				1000000000,
				451,
				vec![],
			),
			Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes
			})
		);
		System::assert_last_event(Event::EVM(crate::Event::ExecutedFailed {
			from: alice(),
			contract: factory_contract_address,
			exit_reason: ExitReason::Error(ExitError::Other(
				Into::<&str>::into(Error::<Runtime>::OutOfStorage).into(),
			)),
			output: vec![],
			logs: vec![],
			used_gas: 1000000000,
			used_storage: 0,
		}));

		// Factory.createContract(2)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000002").unwrap();
		let result = <Runtime as Config>::Runner::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			amount,
			1000000000,
			452,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));

		// 2 * code_size + array_update(2 items) + extra_size = 452, array_length is already set
		let expected_used_storage = 2 * 62 + 2 * 64 + 2 * 100;

		assert_eq!(expected_used_storage, 452);
		assert_eq!(result.used_storage, expected_used_storage);
	});
}

#[test]
fn evm_execute_mode_should_work() {
	// pragma solidity ^0.5.0;

	// contract Factory {
	// 	Contract[] newContracts;

	// 	function createContract (uint num) public payable {
	// 		for(uint i = 0; i < num; i++) {
	// 			Contract newContract = new Contract();
	// 			newContracts.push(newContract);
	// 		}
	// 	}
	// }

	// contract Contract {}
	let contract = from_hex(
		"0x608060405234801561001057600080fd5b506101a0806100206000396000f3fe60806040526004361061001e5760003560e01c80639db8d7d514610023575b600080fd5b61004f6004803603602081101561003957600080fd5b8101908080359060200190929190505050610051565b005b60008090505b8181101561010057600060405161006d90610104565b604051809103906000f080158015610089573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff16021790555050508080600101915050610057565b5050565b605b806101118339019056fe6080604052348015600f57600080fd5b50603e80601d6000396000f3fe6080604052600080fdfea265627a7a7231582035666e9471716d6d05ed9f0c1ab13d0371f49d536270f905bff06cd98212dcb064736f6c63430005110032a265627a7a723158203b6aaf6588bc3e6a35986612a62f715255430eab09ffb24401e5f18eb58a05d564736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		let mut alice_balance = INITIAL_BALANCE - 516 * EVM::get_storage_deposit_per_byte();

		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract.clone(),
			0,
			1000000000,
			1000000000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let account = Accounts::<Runtime>::get(&result.value).unwrap();
		let code_info = CodeInfos::<Runtime>::get(account.contract_info.unwrap().code_hash).unwrap();

		// code_size + extra_size = 516
		let expected_used_storage = 416 + 100;
		assert_eq!(code_info.code_size, 416);
		assert_eq!(result.used_storage, expected_used_storage);
		assert_eq!(balance(alice()), alice_balance);
		let factory_contract_address = result.value;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(factory_contract_address);

		let context = InvokeContext {
			contract: factory_contract_address,
			sender: alice(),
			origin: alice(),
		};

		// ExecutionMode::EstimateGas
		// Factory.createContract(1)
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let result = EVM::execute(
			context,
			create_contract,
			Default::default(),
			2_100_000,
			1000,
			ExecutionMode::EstimateGas,
		)
		.unwrap();
		// code_size + array_update(1 item + length) + extra_size = 290
		let expected_used_storage = 62 + 2 * 64 + 100;
		assert_eq!(expected_used_storage, 290);
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				value: vec![],
				used_gas: U256::from(142_445),
				used_storage: expected_used_storage,
				logs: vec![]
			}
		);

		// Factory.createContract(2)
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000002").unwrap();
		let result = EVM::execute(
			context,
			create_contract,
			Default::default(),
			2_100_000,
			2_100_000,
			ExecutionMode::EstimateGas,
		)
		.unwrap();

		// 2 * code_size + array_update(2 items + length) + extra_size = 516
		let expected_used_storage = 2 * 62 + 3 * 64 + 2 * 100;
		assert_eq!(expected_used_storage, 516);
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				value: vec![],
				used_gas: U256::from(259_561),
				used_storage: expected_used_storage,
				logs: vec![]
			}
		);
		assert_eq!(balance(alice()), alice_balance);

		let code_hash = Accounts::<Runtime>::get(&factory_contract_address)
			.unwrap()
			.contract_info
			.unwrap()
			.code_hash;

		let contract_storage_size = ContractStorageSizes::<Runtime>::get(&factory_contract_address);
		let storage_count = AccountStorages::<Runtime>::iter_prefix(&factory_contract_address).count() as u32;
		let code_info = CodeInfos::<Runtime>::get(&code_hash).unwrap();
		assert_eq!(code_info.code_size, 416);
		assert_eq!(
			contract_storage_size,
			NEW_CONTRACT_EXTRA_BYTES + code_info.code_size + storage_count * 64
		);
		assert_eq!(storage_count, 0);

		// ExecutionMode::Execute
		// Factory.createContract(1)
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		assert_noop!(
			EVM::execute(
				context,
				create_contract,
				Default::default(),
				2_100_000,
				0,
				ExecutionMode::Execute,
			),
			Error::<Runtime>::OutOfStorage
		);
		assert_eq!(balance(alice()), alice_balance);

		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let result = EVM::execute(
			context,
			create_contract,
			Default::default(),
			2_100_000,
			2_100_000,
			ExecutionMode::Execute,
		)
		.unwrap();

		// code_size + array_update(1 item + length) + extra_size = 290
		let expected_used_storage = 62 + 2 * 64 + 100;
		assert_eq!(expected_used_storage, 290);
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				value: vec![],
				used_gas: U256::from(110_469),
				used_storage: expected_used_storage,
				logs: vec![]
			}
		);

		let contract_storage_size = ContractStorageSizes::<Runtime>::get(&factory_contract_address);
		let storage_count = AccountStorages::<Runtime>::iter_prefix(&factory_contract_address).count() as u32;
		let code_info = CodeInfos::<Runtime>::get(&code_hash).unwrap();
		assert_eq!(code_info.code_size, 416);
		assert_eq!(
			contract_storage_size,
			NEW_CONTRACT_EXTRA_BYTES + code_info.code_size + storage_count * 64
		);
		// one address stored in array + array_length
		assert_eq!(storage_count, 2);

		alice_balance -= expected_used_storage as u128 * EVM::get_storage_deposit_per_byte();

		assert_eq!(balance(alice()), alice_balance);

		// ExecutionMode::View
		// Discard any state changes
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let result = EVM::execute(
			context,
			create_contract,
			Default::default(),
			2_100_000,
			2_100_000,
			ExecutionMode::View,
		)
		.unwrap();

		// code_size + array_update(1 item) + extra_size = 226, array_length is already set
		let expected_used_storage = 62 + 64 + 100;
		assert_eq!(expected_used_storage, 226);
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				value: vec![],
				used_gas: U256::from(93_369),
				used_storage: expected_used_storage,
				logs: vec![]
			}
		);

		assert_eq!(balance(alice()), alice_balance);
	});
}

#[test]
fn should_update_storage() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//     mapping(address => uint256) public values;
	//
	//     constructor() public {
	//         values[msg.sender] = 42;
	//     }
	//
	//     function set(uint val) public {
	//      values[msg.sender] = val;
	//     }
	// }

	let contract = from_hex(
		"0x608060405234801561001057600080fd5b50602a6000803373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002081905550610154806100646000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c806354fe9fd71461003b57806360fe47b114610093575b600080fd5b61007d6004803603602081101561005157600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff1690602001909291905050506100c1565b6040518082815260200191505060405180910390f35b6100bf600480360360208110156100a957600080fd5b81019080803590602001909291905050506100d9565b005b60006020528060005260406000206000915090505481565b806000803373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055505056fea265627a7a723158207ab6991e97c9c12f57d81df0c7f955435418354adeb26116b581d7f2f035ca8f64736f6c63430005110032"
	).unwrap();

	new_test_ext().execute_with(|| {
		// create contract
		let result = <Runtime as Config>::Runner::create(
			alice(),
			contract,
			0,
			500000,
			100000,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		let contract_address = result.value;

		let code_size = 340u32;

		let mut used_storage = code_size + NEW_CONTRACT_EXTRA_BYTES + STORAGE_SIZE;

		assert_eq!(result.used_storage, used_storage as i32);

		assert_eq!(ContractStorageSizes::<Runtime>::get(&contract_address), used_storage);

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		publish_free(contract_address);

		// call method `set(123)`
		let bob_account_id = <Runtime as Config>::AddressMapping::get_account_id(&bob());
		assert_eq!(
			EVM::call(
				Origin::signed(bob_account_id),
				contract_address,
				from_hex("0x60fe47b1000000000000000000000000000000000000000000000000000000000000007b").unwrap(),
				0,
				1000000,
				0,
				vec![],
			),
			Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes
			})
		);
		System::assert_last_event(Event::EVM(crate::Event::ExecutedFailed {
			from: bob(),
			contract: contract_address,
			exit_reason: ExitReason::Error(ExitError::Other(
				Into::<&str>::into(Error::<Runtime>::OutOfStorage).into(),
			)),
			output: vec![],
			logs: vec![],
			used_gas: 1000000,
			used_storage: 0,
		}));

		// call method `set(123)`
		let result = <Runtime as Config>::Runner::call(
			bob(),
			alice(),
			contract_address,
			from_hex("0x60fe47b1000000000000000000000000000000000000000000000000000000000000007b").unwrap(),
			0,
			1000000,
			STORAGE_SIZE,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		used_storage += STORAGE_SIZE;

		assert_eq!(result.used_storage, STORAGE_SIZE as i32);
		assert_eq!(ContractStorageSizes::<Runtime>::get(&contract_address), used_storage);

		// call method `set(0)`
		let result = <Runtime as Config>::Runner::call(
			bob(),
			alice(),
			contract_address,
			from_hex("0x60fe47b10000000000000000000000000000000000000000000000000000000000000000").unwrap(),
			0,
			1000000,
			STORAGE_SIZE,
			vec![],
			<Runtime as Config>::config(),
		)
		.unwrap();

		used_storage -= STORAGE_SIZE;

		assert_eq!(result.used_storage, -(STORAGE_SIZE as i32));
		assert_eq!(ContractStorageSizes::<Runtime>::get(&contract_address), used_storage);
	});
}

#[test]
fn code_hash_with_non_existent_address_should_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			EVM::code_hash_at_address(&H160::from_str("0x0000000000000000000000000000000000000000").unwrap()),
			code_hash(&[])
		);
	});
}

#[test]
fn convert_decimals_should_not_work() {
	let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());

	new_test_ext().execute_with(|| {
		assert_eq!(
			EVM::create(
				Origin::signed(alice_account_id.clone()),
				vec![],
				1,
				1000000,
				1000000,
				vec![]
			),
			Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes
			})
		);
		System::assert_last_event(Event::EVM(crate::Event::CreatedFailed {
			from: alice(),
			contract: H160::default(),
			exit_reason: ExitReason::Error(ExitError::Other(
				Into::<&str>::into(Error::<Runtime>::InvalidDecimals).into(),
			)),
			logs: vec![],
			used_gas: 1000000,
			used_storage: 0,
		}));
		assert_eq!(
			EVM::create2(
				Origin::signed(alice_account_id.clone()),
				vec![],
				H256::default(),
				1,
				1000000,
				1000000,
				vec![],
			),
			Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes
			})
		);
		System::assert_last_event(Event::EVM(crate::Event::CreatedFailed {
			from: alice(),
			contract: H160::default(),
			exit_reason: ExitReason::Error(ExitError::Other(
				Into::<&str>::into(Error::<Runtime>::InvalidDecimals).into(),
			)),
			logs: vec![],
			used_gas: 1000000,
			used_storage: 0,
		}));
		assert_eq!(
			EVM::call(
				Origin::signed(alice_account_id.clone()),
				H160::default(),
				vec![],
				1,
				1000000,
				1000000,
				vec![],
			),
			Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes
			})
		);
		System::assert_last_event(Event::EVM(crate::Event::ExecutedFailed {
			from: alice(),
			contract: H160::default(),
			exit_reason: ExitReason::Error(ExitError::Other(
				Into::<&str>::into(Error::<Runtime>::InvalidDecimals).into(),
			)),
			output: vec![],
			logs: vec![],
			used_gas: 1000000,
			used_storage: 0,
		}));
	});
}

#[test]
fn remove_empty_account_works() {
	new_test_ext().execute_with(|| {
		let address = H160::from([1; 20]);
		assert_ok!(Pallet::<Runtime>::remove_account(&address));
	});
}

#[test]
#[should_panic(expected = "removed account while is still linked to contract info")]
fn remove_account_with_provides_should_panic() {
	new_test_ext().execute_with(|| {
		let address = H160::from([1; 20]);
		let code = vec![0x00];
		let code_hash = code_hash(&code);
		Codes::<Runtime>::insert(&code_hash, BoundedVec::try_from(code).unwrap());
		CodeInfos::<Runtime>::insert(
			&code_hash,
			CodeInfo {
				code_size: 1,
				ref_count: 1,
			},
		);
		Accounts::<Runtime>::insert(
			&address,
			AccountInfo {
				nonce: 0,
				contract_info: Some(ContractInfo {
					code_hash,
					maintainer: Default::default(),
					published: false,
				}),
			},
		);
		let _ = Pallet::<Runtime>::remove_account(&address);
	});
}

#[test]
fn remove_account_works() {
	new_test_ext().execute_with(|| {
		let address = H160::from([1; 20]);
		Accounts::<Runtime>::insert(
			&address,
			AccountInfo {
				nonce: 0,
				contract_info: None,
			},
		);
		assert_ok!(Pallet::<Runtime>::remove_account(&address));
		assert_eq!(Accounts::<Runtime>::contains_key(&address), false);
	});
}

#[test]
fn auto_publish_works() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/CreateContractFactory.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	new_test_ext().execute_with(|| {
		let alice_account_id = <Runtime as Config>::AddressMapping::get_account_id(&alice());
		assert_ok!(EVM::create(
			Origin::signed(alice_account_id.clone()),
			code,
			0,
			2_100_000,
			10000,
			vec![]
		));

		let factory = H160::from_str("0x5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d").unwrap();
		System::assert_last_event(Event::EVM(crate::Event::Created {
			from: alice(),
			contract: factory,
			logs: vec![],
			used_gas: 593209,
			used_storage: 2609,
		}));

		// call method `createContract()`
		assert_ok!(EVM::call(
			Origin::signed(alice_account_id.clone()),
			factory,
			from_hex("0x412a5a6d").unwrap(),
			0,
			1000000,
			10000,
			vec![],
		));
		System::assert_last_event(Event::EVM(crate::Event::Executed {
			from: alice(),
			contract: factory,
			logs: vec![
				crate::Log {
					address: H160::from_str("0x7b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap(),
					topics: vec![
						H256::from_str("0xb0199510a4d57fac89f9b613861450ae948394f2abe3bf9918eb3c6890243f00").unwrap(),
						H256::from_str("0x00000000000000000000000030f612c54706d40f65acaf10b8f6989103c2af58").unwrap(),
					],
					data: vec![],
				},
				crate::Log {
					address: factory,
					topics: vec![
						H256::from_str("0x6837ff1e738d95fc8bb5f12ce1513f42866f6c59c226c77342c4f36a1958ea10").unwrap(),
						H256::from_str("0x0000000000000000000000007b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap(),
					],
					data: vec![],
				},
			],
			used_gas: 387664,
			used_storage: 1530,
		}));

		assert_eq!(
			EVM::accounts(factory).unwrap().contract_info,
			Some(ContractInfo {
				code_hash: H256::from_str("0xd007bd109daec7dec73d897c079b67b3d2fd6ad4892a916c5e03e21bb60ff384")
					.unwrap(),
				maintainer: alice(),
				published: false
			})
		);
		assert_eq!(
			EVM::accounts(H160::from_str("0x7b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap())
				.unwrap()
				.contract_info,
			Some(ContractInfo {
				code_hash: H256::from_str("0xe12fa7753d9cd8de1f8b597fef33ab91c2749fe4a1022b648f949ab2566f391f")
					.unwrap(),
				maintainer: factory,
				published: false
			})
		);
		assert_eq!(
			EVM::accounts(H160::from_str("0x30f612c54706d40f65acaf10b8f6989103c2af58").unwrap())
				.unwrap()
				.contract_info,
			Some(ContractInfo {
				code_hash: H256::from_str("0x46460b564756d0e02bbfdc8fc3d47d1a68c3b3d8301b5de90da83d6d75e0b6c7")
					.unwrap(),
				maintainer: H160::from_str("0x7b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap(),
				published: false
			})
		);

		// publish the factory
		assert_ok!(EVM::publish_free(Origin::signed(CouncilAccount::get()), factory));

		// call method `createContract()`
		assert_ok!(EVM::call(
			Origin::signed(alice_account_id.clone()),
			factory,
			from_hex("0x412a5a6d").unwrap(),
			0,
			1000000,
			10000,
			vec![],
		));
		System::assert_last_event(Event::EVM(crate::Event::Executed {
			from: alice(),
			contract: factory,
			logs: vec![
				crate::Log {
					address: H160::from_str("0x39b26a36a8a175ce7d498b5ef187d1ab2f381bbd").unwrap(),
					topics: vec![
						H256::from_str("0xb0199510a4d57fac89f9b613861450ae948394f2abe3bf9918eb3c6890243f00").unwrap(),
						H256::from_str("0x000000000000000000000000769a55efaf4dbdd6f44efce668455522b61abb82").unwrap(),
					],
					data: vec![],
				},
				crate::Log {
					address: factory,
					topics: vec![
						H256::from_str("0x6837ff1e738d95fc8bb5f12ce1513f42866f6c59c226c77342c4f36a1958ea10").unwrap(),
						H256::from_str("0x00000000000000000000000039b26a36a8a175ce7d498b5ef187d1ab2f381bbd").unwrap(),
					],
					data: vec![],
				},
			],
			used_gas: 370564,
			used_storage: 1466,
		}));

		assert_eq!(
			EVM::accounts(factory).unwrap().contract_info,
			Some(ContractInfo {
				code_hash: H256::from_str("0xd007bd109daec7dec73d897c079b67b3d2fd6ad4892a916c5e03e21bb60ff384")
					.unwrap(),
				maintainer: alice(),
				published: true
			})
		);
		assert_eq!(
			EVM::accounts(H160::from_str("0x39b26a36a8a175ce7d498b5ef187d1ab2f381bbd").unwrap())
				.unwrap()
				.contract_info,
			Some(ContractInfo {
				code_hash: H256::from_str("0xe12fa7753d9cd8de1f8b597fef33ab91c2749fe4a1022b648f949ab2566f391f")
					.unwrap(),
				maintainer: H160::from_str("0x5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d").unwrap(),
				published: true
			})
		);
		assert_eq!(
			EVM::accounts(H160::from_str("0x769a55efaf4dbdd6f44efce668455522b61abb82").unwrap())
				.unwrap()
				.contract_info,
			Some(ContractInfo {
				code_hash: H256::from_str("0x46460b564756d0e02bbfdc8fc3d47d1a68c3b3d8301b5de90da83d6d75e0b6c7")
					.unwrap(),
				maintainer: H160::from_str("0x39b26a36a8a175ce7d498b5ef187d1ab2f381bbd").unwrap(),
				published: true
			})
		);

		// call method `callContract()`
		assert_ok!(EVM::call(
			Origin::signed(alice_account_id.clone()),
			factory,
			from_hex("0x0f24df3a").unwrap(),
			0,
			1000000,
			10000,
			vec![],
		));
		System::assert_last_event(Event::EVM(crate::Event::Executed {
			from: alice(),
			contract: factory,
			logs: vec![crate::Log {
				address: H160::from_str("0x7b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap(),
				topics: vec![
					H256::from_str("0xb0199510a4d57fac89f9b613861450ae948394f2abe3bf9918eb3c6890243f00").unwrap(),
					H256::from_str("0x000000000000000000000000d8a09b53762a01c2beb363d5355f4eecf7b48360").unwrap(),
				],
				data: vec![],
			}],
			used_gas: 147214,
			used_storage: 407,
		}));

		assert_eq!(
			EVM::accounts(H160::from_str("d8a09b53762a01c2beb363d5355f4eecf7b48360").unwrap())
				.unwrap()
				.contract_info,
			Some(ContractInfo {
				code_hash: H256::from_str("0x46460b564756d0e02bbfdc8fc3d47d1a68c3b3d8301b5de90da83d6d75e0b6c7")
					.unwrap(),
				maintainer: H160::from_str("0x7b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap(),
				published: true
			})
		);
	});
}
