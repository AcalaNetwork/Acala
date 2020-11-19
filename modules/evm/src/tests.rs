#![cfg(test)]

use super::*;

use frame_support::{assert_ok, impl_outer_dispatch, impl_outer_origin, parameter_types};
use primitives::{Amount, BlockNumber, CurrencyId, TokenSymbol};
use sp_core::bytes::{from_hex, to_hex};
use sp_core::{Blake2Hasher, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};
use std::{collections::BTreeMap, str::FromStr};

impl_outer_origin! {
	pub enum Origin for Test where system = frame_system {}
}

impl_outer_dispatch! {
	pub enum OuterCall for Test where origin: Origin {
		self::EVM,
	}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Trait for Test {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Call = OuterCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Trait for Test {
	type Balance = u64;
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 1000;
}
impl pallet_timestamp::Trait for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl orml_tokens::Trait for Test {
	type Event = ();
	type Balance = u64;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
	type WeightInfo = ();
}
pub type Tokens = orml_tokens::Module<Test>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
}

impl orml_currencies::Trait for Test {
	type Event = ();
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type Currencies = orml_currencies::Module<Test>;
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;

pub struct MockAccountMapping<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> AccountMapping<AccountId32> for MockAccountMapping<T>
where
	T::AccountId: From<AccountId32>,
{
	fn into_h160(_account_id: AccountId32) -> H160 {
		H160::default()
	}
}

impl Trait for Test {
	type CallOrigin = EnsureAddressRoot<Self::AccountId>;

	type AddressMapping = HashedAddressMapping<Blake2Hasher>;
	type AccountMapping = MockAccountMapping<Test>;
	type Currency = Balances;
	type MergeAccount = Currencies;

	type Event = Event<Test>;
	type Precompiles = ();
	type ChainId = SystemChainId;
	type Runner = crate::runner::native::Runner<Self>;
}

type System = frame_system::Module<Test>;
type Balances = pallet_balances::Module<Test>;
type EVM = Module<Test>;

const INITIAL_BALANCE: u64 = 1_000_000_000_000;

fn alice() -> H160 {
	H160::from_str("1000000000000000000000000000000000000001").unwrap()
}

fn bob() -> H160 {
	H160::from_str("1000000000000000000000000000000000000002").unwrap()
}

fn charlie() -> H160 {
	H160::from_str("1000000000000000000000000000000000000003").unwrap()
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	let mut accounts = BTreeMap::new();
	accounts.insert(
		alice(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: vec![
				0x00, // STOP
			],
		},
	);
	accounts.insert(
		bob(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: vec![
				0xff, // INVALID
			],
		},
	);

	pallet_balances::GenesisConfig::<Test>::default()
		.assimilate_storage(&mut t)
		.unwrap();
	GenesisConfig::<Test> { accounts }.assimilate_storage(&mut t).unwrap();
	t.into()
}

fn balance(address: H160) -> u64 {
	let account_id = <Test as Trait>::AddressMapping::into_account_id(address);
	Balances::free_balance(account_id)
}

#[test]
fn fail_call_return_ok() {
	new_test_ext().execute_with(|| {
		assert_ok!(EVM::call(
			Origin::root(),
			H160::default(),
			alice(),
			Vec::new(),
			U256::default(),
			1000000,
		));

		assert_ok!(EVM::call(
			Origin::root(),
			H160::default(),
			bob(),
			Vec::new(),
			U256::default(),
			1000000,
		));
	});
}

#[test]
fn should_calculate_contract_address() {
	new_test_ext().execute_with(|| {
		let addr = H160::from_str("bec02ff0cbf20042a37d964c33e89f1a2be7f068").unwrap();

		let vicinity = Vicinity {
			gas_price: U256::one(),
			origin: addr,
		};

		let config = <Test as Trait>::config();

		let handler = crate::runner::native::Handler::<Test>::new_with_precompile(
			&vicinity,
			10000usize,
			false,
			config,
			<Test as Trait>::Precompiles::execute,
		);

		assert_eq!(
			handler.create_address(evm::CreateScheme::Legacy { caller: addr }),
			H160::from_str("d654cB21c05cb14895baae28159b1107e9DbD6E4").unwrap()
		);

		handler.inc_nonce(addr);
		assert_eq!(
			handler.create_address(evm::CreateScheme::Legacy { caller: addr }),
			H160::from_str("97784910F057B07bFE317b0552AE23eF34644Aed").unwrap()
		);

		handler.inc_nonce(addr);
		assert_eq!(
			handler.create_address(evm::CreateScheme::Legacy { caller: addr }),
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
		let result = <Test as Trait>::Runner::create(
			caller.clone(),
			contract,
			U256::default(),
			1000000,
		).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let contract_address = result.address;

		assert_eq!(contract_address, H160::from_str("5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d").unwrap());

		assert_eq!(Module::<Test>::account_basic(&caller).nonce, U256::from_str("02").unwrap());

		// multiply(2, 3)
		let multiply = from_hex("0x165c4a1600000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000003").unwrap();

		// call method `multiply`
		let result = <Test as Trait>::Runner::call(
			alice(),
			contract_address,
			multiply,
			U256::default(),
			1000000
		).unwrap();
		assert_eq!(
			U256::from(from_hex("0x06").unwrap().as_slice()),
			U256::from(result.value.as_slice())
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
		let result = <Test as Trait>::Runner::create(alice(), contract, U256::default(), 12_000_000).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert!(String::from_utf8_lossy(&result.value).contains("error message"));
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
		let result = <Test as Trait>::Runner::create(
			caller,
			contract,
			U256::default(),
			1000000,
		).unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		assert_eq!(balance(alice()), INITIAL_BALANCE);

		let contract_address = result.address;

		// call method `foo`
		let foo = from_hex("0xc2985578").unwrap();
		let result = <Test as Trait>::Runner::call(
			caller,
			contract_address,
			foo,
			U256::default(),
			1000000
		).unwrap();

		assert_eq!(balance(alice()), INITIAL_BALANCE);
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert_eq!(
			to_hex(&result.value, true),
			"0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676500000000000000000000000000000000000000"
		);

		let message  = String::from_utf8_lossy(&result.value);
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

		let result = <Test as Trait>::Runner::create(alice(), contract, amount.into(), 100000).unwrap();
		let contract_address = result.address;

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(balance(alice()), INITIAL_BALANCE - amount);
		assert_eq!(balance(contract_address), amount);

		// call getValue()
		let result = <Test as Trait>::Runner::call(
			alice(),
			contract_address,
			from_hex("0x20965255").unwrap(),
			amount.into(),
			100000,
		)
		.unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.value, stored_value);
		assert_eq!(balance(alice()), INITIAL_BALANCE - 2 * amount);
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

		let result =
			<Test as Trait>::Runner::create(alice(), contract, 0.into(), 10000000).expect("create shouldn't fail");
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		let contract_address = result.address;

		// send via transfer
		let mut via_transfer = Vec::from(from_hex("0x636e082b000000000000000000000000").unwrap());
		via_transfer.append(&mut Vec::from(H160::from(charlie()).as_bytes()));

		let result = <Test as Trait>::Runner::call(alice(), contract_address, via_transfer, amount.into(), 1000000)
			.expect("call shouldn't fail");

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(alice()), INITIAL_BALANCE - 1 * amount);
		assert_eq!(balance(charlie()), 1 * amount);

		// send via transfer
		let mut via_send = from_hex("0x74be4806000000000000000000000000").unwrap();
		via_send.append(&mut Vec::from(H160::from(charlie()).as_bytes()));

		let result = <Test as Trait>::Runner::call(alice(), contract_address, via_send, amount.into(), 1000000)
			.expect("call shouldn't fail");

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(charlie()), 2 * amount);
		assert_eq!(balance(alice()), INITIAL_BALANCE - 2 * amount);

		// send via call
		let mut via_call = from_hex("0x830c29ae000000000000000000000000").unwrap();
		via_call.append(&mut Vec::from(H160::from(charlie()).as_bytes()));

		let result = <Test as Trait>::Runner::call(alice(), contract_address, via_call, amount.into(), 1000000)
			.expect("call shouldn't fail");

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
		assert_eq!(balance(charlie()), 3 * amount);
		assert_eq!(balance(alice()), INITIAL_BALANCE - 3 * amount);
	})
}

#[test]
fn contract_should_deploy_contracts() {
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
		let result = <Test as Trait>::Runner::create(alice(), contract.clone(), U256::default(), 1000000000).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		// Factory.createContract
		let create_contract = from_hex("0x412a5a6d").unwrap();
		let result =
			<Test as Trait>::Runner::call(alice(), result.address, create_contract, U256::default(), 1000000000)
				.unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Stopped));
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
		let result = <Test as Trait>::Runner::create(alice(), contract, U256::default(), 12_000_000).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
		assert_eq!(result.used_gas.as_u64(), 95_203u64);
	});
}
