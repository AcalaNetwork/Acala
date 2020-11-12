#![cfg(test)]

use super::*;

use frame_support::{assert_ok, impl_outer_dispatch, impl_outer_origin, parameter_types};
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

impl Trait for Test {
	type CallOrigin = EnsureAddressRoot<Self::AccountId>;
	type WithdrawOrigin = EnsureAddressNever<Self::AccountId>;

	type AddressMapping = HashedAddressMapping<Blake2Hasher>;
	type Currency = Balances;

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
			alice(),
			H160::default(),
			Vec::new(),
			U256::default(),
			1000000,
		));

		assert_ok!(EVM::call(
			Origin::root(),
			bob(),
			H160::default(),
			Vec::new(),
			U256::default(),
			1000000,
		));
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
		let result = <Test as Trait>::Runner::create(
			alice(),
			contract,
			U256::default(),
			1000000,
		).unwrap();
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let contract_address = H160::from(result.value);

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
	});
}

#[test]
fn should_revert() {
	// pragma solidity ^0.5.0;
	//
	// contract Test {
	// 	function foo() public pure {
	// 		require(false, "error message");
	// 	}
	// }
	let contract = from_hex("0x608060405234801561001057600080fd5b5060df8061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063c298557814602d575b600080fd5b60336035565b005b600060a8576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040180806020018281038252600d8152602001807f6572726f72206d6573736167650000000000000000000000000000000000000081525060200191505060405180910390fd5b56fea265627a7a7231582066b3ee33bedba8a318d0d66610145030fdc0f982b11f5160d366e15e4d8ba2ef64736f6c63430005110032").unwrap();

	new_test_ext().execute_with(|| {
		// deploy contract
		let result = <Test as Trait>::Runner::create(
			alice(),
			contract,
			U256::default(),
			1000000,
		).unwrap();

		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		let new_balance = INITIAL_BALANCE - result.used_gas.as_u64();
		assert_eq!(balance(alice()), new_balance);

		let contract_address = H160::from(result.value);

		// call method `foo`
		let foo = from_hex("0xc2985578").unwrap();
		let result = <Test as Trait>::Runner::call(
			alice(),
			contract_address,
			foo,
			U256::default(),
			1000000
		).unwrap();

		assert_eq!(balance(alice()), new_balance - result.used_gas.as_u64());
		assert_eq!(result.exit_reason, ExitReason::Revert(ExitRevert::Reverted));
		assert_eq!(
			to_hex(&result.value, true),
			"0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676500000000000000000000000000000000000000"
		);

		let message  = String::from_utf8_lossy(&result.value);
		assert!(message.contains("error message"));
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
	// }
	let contract = from_hex("0x60806040526040516087380380608783398181016040526020811015602357600080fd5b81019080805190602001909291905050508060008190555050603e8060496000396000f3fe6080604052600080fdfea265627a7a72315820ca74d7bda13b4991ba0b903e13a6d07d5ace341dcea7cfcfc0ba5baad347687764736f6c6343000511003200000000000000000000000000000000000000000000000000000000000003e8").unwrap();
	let transfer_amount = 1000;
	new_test_ext().execute_with(|| {
		let result = <Test as Trait>::Runner::create(alice(), contract, U256::from(transfer_amount), 100000).unwrap();

		let new_balance = INITIAL_BALANCE - result.used_gas.as_u64() - transfer_amount;
		assert_eq!(balance(alice()), new_balance);
		assert_eq!(balance(result.value), transfer_amount);
	});
}

#[test]
fn should_work_with_factory() {
	// 	pragma solidity ^0.5.0;
	//
	// 	contract Factory {
	// 		Contract[] newContracts;
	//
	// 		function createContract () public {
	//	 		Contract newContract = new Contract();
	// 			newContracts.push(newContract);
	// 		}
	// 	}
	//
	// 	contract Contract { }
	let contract = from_hex("0x608060405234801561001057600080fd5b5061016c806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063412a5a6d14610030575b600080fd5b61003861003a565b005b6000604051610048906100d0565b604051809103906000f080158015610064573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b605b806100dd8339019056fe6080604052348015600f57600080fd5b50603e80601d6000396000f3fe6080604052600080fdfea265627a7a723158207237f48b79cb8bd7892b1affe7326666b6104cf050c92e1aea23b4bdcfbc928764736f6c63430005110032a265627a7a72315820ff3b7b0c5ca8cba09847804f4a67b249035d38282c40eeb087d58a9ac661529764736f6c63430005110032").unwrap();
	new_test_ext().execute_with(|| {
		// deploy contract
		let result = <Test as Trait>::Runner::create(alice(), contract, U256::default(), 1000000).unwrap();
		println!("{:?}", result);
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));

		// Factory.createContract(name)
		let create_contract = from_hex("0x412a5a6d").unwrap();
		let result =
			<Test as Trait>::Runner::call(alice(), result.value, create_contract, U256::default(), 10000000).unwrap();
		println!("{:?}", result);
		assert_eq!(result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned));
	});
}
