#![cfg(test)]

use super::*;
use mock::{Event, *};

use crate::runner::handler::Handler;
use frame_support::{assert_noop, assert_ok};
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
		let signer: AccountId32 = AccountId32::from(data).into();

		let origin = Origin::signed(signer);
		assert_ok!(EVM::call(origin.clone(), contract_a(), Vec::new(), 0, 1000000, 0));
		assert_ok!(EVM::call(origin, contract_b(), Vec::new(), 0, 1000000, 0));
	});
}

#[test]
fn should_calculate_contract_address() {
	new_test_ext().execute_with(|| {
		let addr = H160::from_str("bec02ff0cbf20042a37d964c33e89f1a2be7f068").unwrap();

		assert_eq!(
			Handler::<Test>::create_address(evm::CreateScheme::Legacy { caller: addr }),
			H160::from_str("d654cB21c05cb14895baae28159b1107e9DbD6E4").unwrap()
		);

		Handler::<Test>::inc_nonce(addr);
		assert_eq!(
			Handler::<Test>::create_address(evm::CreateScheme::Legacy { caller: addr }),
			H160::from_str("97784910F057B07bFE317b0552AE23eF34644Aed").unwrap()
		);

		Handler::<Test>::inc_nonce(addr);
		assert_eq!(
			Handler::<Test>::create_address(evm::CreateScheme::Legacy { caller: addr }),
			H160::from_str("82155a21E0Ccaee9D4239a582EB2fDAC1D9237c5").unwrap()
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
	let contract = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		// deploy contract
		let caller = alice();
		let result = Runner::<Test>::create(
			caller.clone(),
			contract,
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(contract_address);

		assert_eq!(contract_address, H160::from_str("5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d").unwrap());

		assert_eq!(Module::<Test>::account_basic(&caller).nonce, U256::from_str("02").unwrap());

		// multiply(2, 3)
		let multiply = from_hex("0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003").unwrap();

		// call method `multiply`
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			multiply,
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		).unwrap();
		assert_eq!(
			U256::from(from_hex("0x06").unwrap().as_slice()),
			U256::from(result.output.as_slice())
		);

		assert_eq!(Module::<Test>::account_basic(&caller).nonce, U256::from_str("03").unwrap());

		assert_eq!(Module::<Test>::account_basic(&contract_address).nonce, U256::from_str("01").unwrap());
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
	let contract = from_hex("0x6080604052348015600f57600080fd5b5060006083576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040180806020018281038252600d8152602001807f6572726f72206d6573736167650000000000000000000000000000000000000081525060200191505060405180910390fd5b603e8060906000396000f3fe6080604052600080fdfea265627a7a723158204741083d83bf4e3ee8099dd0b3471c81061237c2e8eccfcb513dfa4c04634b5b64736f6c63430005110032").expect("invalid hex");
	new_test_ext().execute_with(|| {
		let result =
			Runner::<Test>::create(alice(), contract, 0, 12_000_000, 12_000_000, <Test as Config>::config()).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert!(String::from_utf8_lossy(&result.output).contains("error message"));
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
	let contract = from_hex("0x608060405234801561001057600080fd5b5060df8061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c298557814602d575b600080fd5b60336035565b005b600060a8576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040180806020018281038252600d8152602001807f6572726f72206d6573736167650000000000000000000000000000000000000081525060200191505060405180910390fd5b56fea265627a7a7231582066b3ee33bedba8a318d0d66610145030fdc0f982b11f5160d366e15e4d8ba2ef64736f6c63430005110032").unwrap();
	let caller = alice();

	new_test_ext().execute_with(|| {
		// deploy contract
		let result = Runner::<Test>::create(
			caller,
			contract,
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		).unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let alice_balance = INITIAL_BALANCE - 323 * <Test as Config>::StorageDepositPerByte::get();

		assert_eq!(balance(alice()), alice_balance);

		let contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(contract_address);

		// call method `foo`
		let foo = from_hex("0xc2985578").unwrap();
		let result = Runner::<Test>::call(
			caller,
			caller,
			contract_address,
			foo,
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		).unwrap();

		assert_eq!(balance(alice()), alice_balance);
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert_eq!(
			to_hex(&result.output, true),
			"0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676500000000000000000000000000000000000000"
		);

		let message  = String::from_utf8_lossy(&result.output);
		assert!(message.contains("error message"));

		assert_eq!(Module::<Test>::account_basic(&caller).nonce, U256::from_str("03").unwrap());
	});
}

#[test]
fn should_deploy_payable_contract() {
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
	let mut contract = from_hex("0x60806040526040516100c73803806100c783398181016040526020811015602557600080fd5b81019080805190602001909291905050508060008190555050607b8061004c6000396000f3fe608060405260043610601c5760003560e01c806320965255146021575b600080fd5b6027603d565b6040518082815260200191505060405180910390f35b6000805490509056fea265627a7a72315820b832564a9db725638dcef03d07bfbdd2dc818020ea359630317e2126e95c314964736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		let amount = 1000u64;

		let stored_value: Vec<u8> =
			from_hex("0x000000000000000000000000000000000000000000000000000000000000007b").unwrap();
		contract.append(&mut stored_value.clone());

		let result =
			Runner::<Test>::create(alice(), contract, amount, 100000, 100000, <Test as Config>::config()).unwrap();
		let contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(contract_address);

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 287);

		let alice_balance = INITIAL_BALANCE - amount - 287 * <Test as Config>::StorageDepositPerByte::get();
		assert_eq!(balance(alice()), alice_balance);
		assert_eq!(balance(contract_address), amount);

		// call getValue()
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			from_hex("0x20965255").unwrap(),
			amount,
			100000,
			100000,
			<Test as Config>::config(),
		)
		.unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.output, stored_value);
		assert_eq!(result.used_storage, 0);

		assert_eq!(balance(alice()), alice_balance - amount);
		assert_eq!(balance(contract_address), 2 * amount);
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
	let contract = from_hex("0x608060405234801561001057600080fd5b50610318806100206000396000f3fe6080604052600436106100345760003560e01c8063636e082b1461003957806374be48061461007d578063830c29ae146100c1575b600080fd5b61007b6004803603602081101561004f57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050610105565b005b6100bf6004803603602081101561009357600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919050505061014f565b005b610103600480360360208110156100d757600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff1690602001909291905050506101ff565b005b8073ffffffffffffffffffffffffffffffffffffffff166108fc349081150290604051600060405180830381858888f1935050505015801561014b573d6000803e3d6000fd5b5050565b60008173ffffffffffffffffffffffffffffffffffffffff166108fc349081150290604051600060405180830381858888f193505050509050806101fb576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260148152602001807f4661696c656420746f2073656e6420457468657200000000000000000000000081525060200191505060405180910390fd5b5050565b600060608273ffffffffffffffffffffffffffffffffffffffff163460405180600001905060006040518083038185875af1925050503d8060008114610261576040519150601f19603f3d011682016040523d82523d6000602084013e610266565b606091505b5091509150816102de576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260148152602001807f4661696c656420746f2073656e6420457468657200000000000000000000000081525060200191505060405180910390fd5b50505056fea265627a7a723158201b401be037c87d59ec386e75b0166702abb5a64f93ea20080904b6791bd88d1564736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		let amount = 1000u64;

		let result = Runner::<Test>::create(alice(), contract, 0, 10000000, 10000000, <Test as Config>::config())
			.expect("create shouldn't fail");
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 892);

		let alice_balance = INITIAL_BALANCE - 892 * <Test as Config>::StorageDepositPerByte::get();
		assert_eq!(balance(alice()), alice_balance);

		let contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(contract_address);

		// send via transfer
		let mut via_transfer = Vec::from(from_hex("0x636e082b").unwrap());
		via_transfer.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			via_transfer,
			amount,
			1000000,
			1000000,
			<Test as Config>::config(),
		)
		.expect("call shouldn't fail");

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(alice()), alice_balance - 1 * amount);
		assert_eq!(balance(charlie()), 1 * amount);

		// send via send
		let mut via_send = from_hex("0x74be4806").unwrap();
		via_send.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			via_send,
			amount,
			1000000,
			1000000,
			<Test as Config>::config(),
		)
		.expect("call shouldn't fail");

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(charlie()), 2 * amount);
		assert_eq!(balance(alice()), alice_balance - 2 * amount);

		// send via call
		let mut via_call = from_hex("0x830c29ae").unwrap();
		via_call.append(&mut Vec::from(H256::from(charlie()).as_bytes()));

		let result = Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			via_call,
			amount,
			1000000,
			1000000,
			<Test as Config>::config(),
		)
		.expect("call shouldn't fail");

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(charlie()), 3 * amount);
		assert_eq!(balance(alice()), alice_balance - 3 * amount);
	})
}

#[test]
fn contract_should_deploy_contracts() {
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
	let contract = from_hex("0x608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a40029").unwrap();
	new_test_ext().execute_with(|| {
		let result = Runner::<Test>::create(
			alice(),
			contract.clone(),
			0,
			1000000000,
			1000000000,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 467);

		let alice_balance = INITIAL_BALANCE - 467 * <Test as Config>::StorageDepositPerByte::get();

		assert_eq!(balance(alice()), alice_balance);
		let factory_contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(factory_contract_address);

		assert_eq!(balance(result.address), 0);
		assert_eq!(
			reserved_balance(result.address),
			467 * <Test as Config>::StorageDepositPerByte::get()
		);

		// Factory.createContract
		let amount = 1000000000;
		let create_contract = from_hex("0x412a5a6d").unwrap();
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			result.address,
			create_contract,
			amount,
			1000000000,
			1000000000,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 281);

		assert_eq!(
			balance(alice()),
			alice_balance - amount - 281 * <Test as Config>::StorageDepositPerByte::get()
		);
		assert_eq!(balance(factory_contract_address), amount);
		let contract_address = H160::from_str("7b8f8ca099f6e33cf1817cf67d0556429cfc54e4").unwrap();
		assert_eq!(reserved_balance(contract_address), 1530);
	});
}

#[test]
fn contract_should_deploy_contracts_without_payable() {
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
	let contract = from_hex("0x608060405234801561001057600080fd5b5061016c806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063412a5a6d14610030575b600080fd5b61003861003a565b005b6000604051610048906100d0565b604051809103906000f080158015610064573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b605b806100dd8339019056fe6080604052348015600f57600080fd5b50603e80601d6000396000f3fe6080604052600080fdfea265627a7a7231582094976cee5af14bf59c4bae67c79c12eb15de19bc18ad6038f3ee0898273c9c0564736f6c63430005110032a265627a7a72315820e19ae28dbf01eae11c526295a1ac533ea341c74d5724efe43171f6010fc98b3964736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		let result = Runner::<Test>::create(
			alice(),
			contract.clone(),
			0,
			1000000000,
			1000000000,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let alice_balance = INITIAL_BALANCE - 464 * <Test as Config>::StorageDepositPerByte::get();

		assert_eq!(balance(alice()), alice_balance);
		let factory_contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(result.address);

		// Factory.createContract
		let create_contract = from_hex("0x412a5a6d").unwrap();
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			result.address,
			create_contract,
			0,
			1000000000,
			1000000000,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 290);
		assert_eq!(
			balance(alice()),
			alice_balance - (result.used_storage as u64 * <Test as Config>::StorageDepositPerByte::get())
		);
		assert_eq!(balance(factory_contract_address), 0);
	});
}

#[test]
fn deploy_factory() {
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
	let contract = from_hex("0x608060405234801561001057600080fd5b5060405161001d90610121565b604051809103906000f080158015610039573d6000803e3d6000fd5b506000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1663c29855786040518163ffffffff1660e01b815260040160206040518083038186803b1580156100e057600080fd5b505afa1580156100f4573d6000803e3d6000fd5b505050506040513d602081101561010a57600080fd5b81019080805190602001909291905050505061012d565b60a58061017983390190565b603e8061013b6000396000f3fe6080604052600080fdfea265627a7a7231582064177030ee644a03aaf8d65027df9e0331c8bc4b161de25bfb8aa3142848e0f864736f6c634300051100326080604052348015600f57600080fd5b5060878061001e6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c298557814602d575b600080fd5b60336049565b6040518082815260200191505060405180910390f35b6000607b90509056fea265627a7a7231582031e5a4abae00962cfe9875df1b5b0d3ce6624e220cb8c714a948794fcddb6b4f64736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		let result = Runner::<Test>::create(alice(), contract, 0, 2_000_000, 5000, <Test as Config>::config()).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_gas.as_u64(), 95_203u64);
		assert_eq!(result.used_storage, 461);
		assert_eq!(
			balance(alice()),
			INITIAL_BALANCE - (result.used_storage as u64 * <Test as Config>::StorageDepositPerByte::get())
		);
	});
}

#[test]
fn create_network_contract_works() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		// deploy contract
		assert_ok!(EVM::create_network_contract(
			Origin::signed(NetworkContractAccount::get()),
			contract,
			0,
			1000000,
			1000000,
		));

		assert_eq!(
			Module::<Test>::account_basic(&NetworkContractSource::get()).nonce,
			U256::from_str("02").unwrap()
		);

		let created_event = Event::evm_mod(crate::Event::Created(H160::from_low_u64_be(NETWORK_CONTRACT_INDEX)));
		assert!(System::events().iter().any(|record| record.event == created_event));

		assert_eq!(EVM::network_contract_index(), NETWORK_CONTRACT_INDEX + 1);
	});
}

#[test]
fn create_network_contract_fails_if_non_network_contract_origin() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		assert_noop!(
			EVM::create_network_contract(
				Origin::signed(AccountId32::from([1u8; 32])),
				contract,
				0,
				1000000,
				1000000
			),
			BadOrigin
		);
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
	let contract = from_hex("0x608060405234801561001057600080fd5b5060405161001d90610121565b604051809103906000f080158015610039573d6000803e3d6000fd5b506000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1663c29855786040518163ffffffff1660e01b815260040160206040518083038186803b1580156100e057600080fd5b505afa1580156100f4573d6000803e3d6000fd5b505050506040513d602081101561010a57600080fd5b81019080805190602001909291905050505061012d565b60a58061017983390190565b603e8061013b6000396000f3fe6080604052600080fdfea265627a7a7231582064177030ee644a03aaf8d65027df9e0331c8bc4b161de25bfb8aa3142848e0f864736f6c634300051100326080604052348015600f57600080fd5b5060878061001e6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c298557814602d575b600080fd5b60336049565b6040518082815260200191505060405180910390f35b6000607b90509056fea265627a7a7231582031e5a4abae00962cfe9875df1b5b0d3ce6624e220cb8c714a948794fcddb6b4f64736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		let result =
			Runner::<Test>::create(alice(), contract, 0, 12_000_000, 12_000_000, <Test as Config>::config()).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 461);
		let alice_balance = INITIAL_BALANCE - 461 * <Test as Config>::StorageDepositPerByte::get();

		assert_eq!(balance(alice()), alice_balance);

		let alice_account_id = <Test as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Test as Config>::AddressMapping::get_account_id(&bob());
		assert_eq!(balance(bob()), INITIAL_BALANCE);
		// transfer_maintainer
		assert_ok!(EVM::transfer_maintainer(
			Origin::signed(alice_account_id.clone()),
			result.address,
			bob()
		));
		let event = Event::evm_mod(crate::Event::TransferredMaintainer(result.address, bob()));
		assert!(System::events().iter().any(|record| record.event == event));
		assert_eq!(balance(bob()), INITIAL_BALANCE);

		assert_noop!(
			EVM::transfer_maintainer(Origin::signed(bob_account_id.clone()), H160::default(), alice()),
			Error::<Test>::ContractNotFound
		);

		assert_noop!(
			EVM::transfer_maintainer(Origin::signed(alice_account_id.clone()), result.address, bob()),
			Error::<Test>::NoPermission
		);
		assert_eq!(balance(alice()), alice_balance);
	});
}

#[test]
fn should_deploy() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		let alice_account_id = <Test as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Test as Config>::AddressMapping::get_account_id(&bob());

		// contract not created yet
		assert_noop!(EVM::deploy(Origin::signed(alice_account_id.clone()), H160::default()), Error::<Test>::ContractNotFound);

		// if the contract not exists, evm will return ExitSucceed::Stopped.
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			EvmAddress::default(),
			vec![],
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 0);

		// create contract
		let result = Runner::<Test>::create(alice(), contract, 0, 21_000_000, 21_000_000, <Test as Config>::config()).unwrap();
		let contract_address = result.address;

		assert_eq!(result.used_storage, 284);
		let alice_balance = INITIAL_BALANCE - 284 * <Test as Config>::StorageDepositPerByte::get();

		assert_eq!(balance(alice()), alice_balance);

		// multiply(2, 3)
		let multiply = from_hex("0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003").unwrap();

		// call method `multiply` will fail, not deployed yet
		assert_noop!(Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		), Error::<Test>::NoPermission);

		// developer can call the undeployed contract
		assert_ok!(EVM::enable_contract_development(Origin::signed(bob_account_id.clone())));
		assert_ok!(Runner::<Test>::call(
			bob(),
			bob(),
			contract_address,
			vec![],
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		));

		// not maintainer
		assert_noop!(EVM::deploy(Origin::signed(bob_account_id), contract_address), Error::<Test>::NoPermission);

		assert_ok!(EVM::deploy(Origin::signed(alice_account_id.clone()), contract_address));
		let code_size = Accounts::<Test>::get(contract_address).map_or(0, |account_info| -> u32 {
			account_info.contract_info.map_or(0, |contract_info| CodeInfos::<Test>::get(contract_info.code_hash).map_or(0, |code_info| code_info.code_size))
		});
		assert_eq!(balance(alice()), INITIAL_BALANCE - DeploymentFee::get() - ((NewContractExtraBytes::get() + code_size) as u64 * StorageDepositPerByte::get()));
		assert_eq!(Balances::free_balance(TreasuryAccount::get()), DeploymentFee::get());

		// call method `multiply` will work
		assert_ok!(Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			multiply,
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		));

		// contract already deployed
		assert_noop!(EVM::deploy(Origin::signed(alice_account_id), contract_address), Error::<Test>::ContractAlreadyDeployed);
	});
}

#[test]
fn should_deploy_free() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		// contract not created yet
		assert_noop!(EVM::deploy_free(Origin::signed(CouncilAccount::get()), H160::default()), Error::<Test>::ContractNotFound);

		// create contract
		let result = Runner::<Test>::create(alice(), contract, 0, 21_000_000, 21_000_000, <Test as Config>::config()).unwrap();
		let contract_address = result.address;

		// multiply(2, 3)
		let multiply = from_hex("0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003").unwrap();

		// call method `multiply` will fail, not deployed yet
		assert_noop!(Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		), Error::<Test>::NoPermission);

		assert_ok!(EVM::deploy_free(Origin::signed(CouncilAccount::get()), contract_address));

		// multiply(2, 3)
		let multiply = from_hex("0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003").unwrap();

		// call method `multiply`
		assert_ok!(Runner::<Test>::call(
			alice(),
			alice(),
			contract_address,
			multiply.clone(),
			0,
			1000000,
			1000000,
			<Test as Config>::config(),
		));

		// contract already deployed
		assert_noop!(EVM::deploy_free(Origin::signed(CouncilAccount::get()), contract_address), Error::<Test>::ContractAlreadyDeployed);
	});
}

#[test]
fn should_enable_contract_development() {
	new_test_ext().execute_with(|| {
		let alice_account_id = <Test as Config>::AddressMapping::get_account_id(&alice());
		assert_ok!(EVM::enable_contract_development(Origin::signed(alice_account_id)));
		assert_eq!(
			Accounts::<Test>::get(alice()).unwrap().developer_deposit,
			Some(DeveloperDeposit::get())
		);
		assert_eq!(balance(alice()), INITIAL_BALANCE - DeveloperDeposit::get());
	});
}

#[test]
fn should_disable_contract_development() {
	new_test_ext().execute_with(|| {
		let alice_account_id = <Test as Config>::AddressMapping::get_account_id(&alice());

		// contract development is not enabled yet
		assert_noop!(
			EVM::disable_contract_development(Origin::signed(alice_account_id.clone())),
			Error::<Test>::ContractDevelopmentNotEnabled
		);
		assert_eq!(balance(alice()), INITIAL_BALANCE);

		// enable contract development
		assert_ok!(EVM::enable_contract_development(Origin::signed(
			alice_account_id.clone()
		)));
		assert_eq!(
			Accounts::<Test>::get(alice()).unwrap().developer_deposit,
			Some(DeveloperDeposit::get())
		);

		// deposit reserved
		assert_eq!(balance(alice()), INITIAL_BALANCE - DeveloperDeposit::get());

		// disable contract development
		assert_ok!(EVM::disable_contract_development(Origin::signed(
			alice_account_id.clone()
		)));
		// deposit unreserved
		assert_eq!(balance(alice()), INITIAL_BALANCE);

		// contract development already disabled
		assert_noop!(
			EVM::disable_contract_development(Origin::signed(alice_account_id)),
			Error::<Test>::ContractDevelopmentNotEnabled
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
	let contract = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();
	let contract_err = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		let alice_account_id = <Test as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Test as Config>::AddressMapping::get_account_id(&bob());

		// create contract
		let result = Runner::<Test>::create(
			alice(),
			contract.clone(),
			0,
			21_000_000,
			21_000_000,
			<Test as Config>::config(),
		)
		.unwrap();
		let contract_address = result.address;
		assert_eq!(result.used_storage, 284);
		let alice_balance = INITIAL_BALANCE - 284 * <Test as Config>::StorageDepositPerByte::get();

		assert_eq!(balance(alice()), alice_balance);

		assert_noop!(
			EVM::set_code(Origin::signed(bob_account_id), contract_address, contract.clone()),
			Error::<Test>::NoPermission
		);
		assert_ok!(EVM::set_code(
			Origin::signed(alice_account_id.clone()),
			contract_address,
			contract.clone()
		));
		assert_ok!(EVM::set_code(Origin::root(), contract_address, contract));

		assert_noop!(
			EVM::set_code(
				Origin::signed(alice_account_id.clone()),
				contract_address,
				contract_err.clone()
			),
			Error::<Test>::ContractExceedsMaxCodeSize
		);

		assert_ok!(EVM::deploy_free(
			Origin::signed(CouncilAccount::get()),
			contract_address
		));

		assert_noop!(
			EVM::set_code(Origin::signed(alice_account_id), contract_address, contract_err),
			Error::<Test>::ContractAlreadyDeployed
		);
	});
}

#[test]
fn should_selfdestruct() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	//	 function multiply(uint a, uint b) public pure returns(uint) {
	// 	 	return a * b;
	// 	 }
	// }
	let contract = from_hex("0x608060405234801561001057600080fd5b5060b88061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063165c4a1614602d575b600080fd5b606060048036036040811015604157600080fd5b8101908080359060200190929190803590602001909291905050506076565b6040518082815260200191505060405180910390f35b600081830290509291505056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d16b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		let alice_account_id = <Test as Config>::AddressMapping::get_account_id(&alice());
		let bob_account_id = <Test as Config>::AddressMapping::get_account_id(&bob());

		// create contract
		let result = Runner::<Test>::create(
			alice(),
			contract.clone(),
			0,
			21_000_000,
			21_000_000,
			<Test as Config>::config(),
		)
		.unwrap();
		let contract_address = result.address;
		assert_eq!(result.used_storage, 284);
		let alice_balance = INITIAL_BALANCE - 284 * <Test as Config>::StorageDepositPerByte::get();

		assert_eq!(balance(alice()), alice_balance);

		assert_noop!(
			EVM::selfdestruct(Origin::signed(bob_account_id), contract_address),
			Error::<Test>::NoPermission
		);
		assert_ok!(EVM::selfdestruct(Origin::signed(alice_account_id), contract_address));
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
	let contract = from_hex("0x608060405234801561001057600080fd5b506101a0806100206000396000f3fe60806040526004361061001e5760003560e01c80639db8d7d514610023575b600080fd5b61004f6004803603602081101561003957600080fd5b8101908080359060200190929190505050610051565b005b60008090505b8181101561010057600060405161006d90610104565b604051809103906000f080158015610089573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff16021790555050508080600101915050610057565b5050565b605b806101118339019056fe6080604052348015600f57600080fd5b50603e80601d6000396000f3fe6080604052600080fdfea265627a7a7231582035666e9471716d6d05ed9f0c1ab13d0371f49d536270f905bff06cd98212dcb064736f6c63430005110032a265627a7a723158203b6aaf6588bc3e6a35986612a62f715255430eab09ffb24401e5f18eb58a05d564736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		let result =
			Runner::<Test>::create(alice(), contract.clone(), 0, 100_000, 1000, <Test as Config>::config()).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 516);
		let alice_balance = INITIAL_BALANCE - 516 * <Test as Config>::StorageDepositPerByte::get();
		assert_eq!(balance(alice()), alice_balance);

		let factory_contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(factory_contract_address);

		assert_eq!(balance(factory_contract_address), 0);
		assert_eq!(
			reserved_balance(factory_contract_address),
			516 * <Test as Config>::StorageDepositPerByte::get()
		);

		// Factory.createContract(1)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			amount,
			1000000000,
			0,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert_eq!(result.used_storage, 0);

		// Factory.createContract(1)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			amount,
			1000000000,
			1000000000,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 290);

		// Factory.createContract(2)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000002").unwrap();
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			amount,
			1000000000,
			127,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert_eq!(result.used_storage, 0);

		// Factory.createContract(2)
		let amount = 1000000000;
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000002").unwrap();
		let result = Runner::<Test>::call(
			alice(),
			alice(),
			factory_contract_address,
			create_contract,
			amount,
			1000000000,
			1000000000,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(result.used_storage, 452);
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
	let contract = from_hex("0x608060405234801561001057600080fd5b506101a0806100206000396000f3fe60806040526004361061001e5760003560e01c80639db8d7d514610023575b600080fd5b61004f6004803603602081101561003957600080fd5b8101908080359060200190929190505050610051565b005b60008090505b8181101561010057600060405161006d90610104565b604051809103906000f080158015610089573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff16021790555050508080600101915050610057565b5050565b605b806101118339019056fe6080604052348015600f57600080fd5b50603e80601d6000396000f3fe6080604052600080fdfea265627a7a7231582035666e9471716d6d05ed9f0c1ab13d0371f49d536270f905bff06cd98212dcb064736f6c63430005110032a265627a7a723158203b6aaf6588bc3e6a35986612a62f715255430eab09ffb24401e5f18eb58a05d564736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		let mut alice_balance = INITIAL_BALANCE - 516 * <Test as Config>::StorageDepositPerByte::get();

		let result = Runner::<Test>::create(
			alice(),
			contract.clone(),
			0,
			1000000000,
			1000000000,
			<Test as Config>::config(),
		)
		.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_storage, 516);
		assert_eq!(balance(alice()), alice_balance);
		let factory_contract_address = result.address;

		#[cfg(not(feature = "with-ethereum-compatibility"))]
		deploy_free(result.address);

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
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				output: vec![],
				used_gas: U256::from(86665),
				used_storage: 290
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
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				output: vec![],
				used_gas: U256::from(173096),
				used_storage: 516
			}
		);
		assert_eq!(balance(alice()), alice_balance);

		// ExecutionMode::Execute
		// Factory.createContract(1)
		let create_contract =
			from_hex("0x9db8d7d50000000000000000000000000000000000000000000000000000000000000001").unwrap();
		let result = EVM::execute(
			context,
			create_contract,
			Default::default(),
			2_100_000,
			0,
			ExecutionMode::Execute,
		)
		.unwrap();
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				output: vec![],
				used_gas: U256::from(44814),
				used_storage: 0
			}
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
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				output: vec![],
				used_gas: U256::from(86665),
				used_storage: 290
			}
		);

		alice_balance -= 290 * <Test as Config>::StorageDepositPerByte::get();

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
		assert_eq!(
			result,
			CallInfo {
				exit_reason: ExitReason::Succeed(ExitSucceed::Stopped),
				output: vec![],
				used_gas: U256::from(71665),
				used_storage: 226
			}
		);

		assert_eq!(balance(alice()), alice_balance);
	});
}
