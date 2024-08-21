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

//! Unit tests for the evm-bridge module.

#![cfg(test)]

use super::*;
use frame_support::{assert_err, assert_noop, assert_ok};
use insta::assert_json_snapshot;
use mock::*;

#[test]
fn should_read_name() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::name(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(
					b"long string name, long string name, long string name, long string name, long string name"
						.to_vec()
				)
			);
		});
}

#[test]
fn should_read_symbol() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::symbol(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(b"TestToken".to_vec())
			);
		});
}

#[test]
fn should_read_decimals() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::decimals(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(17)
			);
		});
}

#[test]
fn should_read_total_supply() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::total_supply(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(ALICE_BALANCE)
			);
		});
}

#[test]
fn should_read_balance_of() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let context = InvokeContext {
				contract: erc20_address(),
				sender: Default::default(),
				origin: Default::default(),
			};

			assert_eq!(EVMBridge::<Runtime>::balance_of(context, bob_evm_addr()), Ok(0));

			assert_eq!(
				EVMBridge::<Runtime>::balance_of(context, alice_evm_addr()),
				Ok(ALICE_BALANCE)
			);

			assert_eq!(EVMBridge::<Runtime>::balance_of(context, bob_evm_addr()), Ok(0));
		});
}

#[test]
fn should_transfer() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000), (bob(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_err!(
				EVMBridge::<Runtime>::transfer(
					InvokeContext {
						contract: erc20_address(),
						sender: bob_evm_addr(),
						origin: bob_evm_addr(),
					},
					alice_evm_addr(),
					10
				),
				Error::<Runtime>::ExecutionRevert
			);

			assert_ok!(EVMBridge::<Runtime>::transfer(
				InvokeContext {
					contract: erc20_address(),
					sender: alice_evm_addr(),
					origin: alice_evm_addr(),
				},
				bob_evm_addr(),
				100
			));
			assert_eq!(
				EVMBridge::<Runtime>::balance_of(
					InvokeContext {
						contract: erc20_address(),
						sender: alice_evm_addr(),
						origin: alice_evm_addr(),
					},
					bob_evm_addr()
				),
				Ok(100)
			);

			assert_ok!(EVMBridge::<Runtime>::transfer(
				InvokeContext {
					contract: erc20_address(),
					sender: bob_evm_addr(),
					origin: bob_evm_addr(),
				},
				alice_evm_addr(),
				10
			));

			assert_eq!(
				EVMBridge::<Runtime>::balance_of(
					InvokeContext {
						contract: erc20_address(),
						sender: alice_evm_addr(),
						origin: bob_evm_addr(),
					},
					bob_evm_addr()
				),
				Ok(90)
			);

			assert_err!(
				EVMBridge::<Runtime>::transfer(
					InvokeContext {
						contract: erc20_address(),
						sender: bob_evm_addr(),
						origin: bob_evm_addr(),
					},
					alice_evm_addr(),
					100
				),
				Error::<Runtime>::ExecutionRevert
			);
		});
}

#[test]
fn liquidation_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_liquidation_ok_contracts();
			let collateral = EvmAddress::from_str("1000000000000000000000000000000000000111").unwrap();
			let repay_dest = EvmAddress::from_str("1000000000000000000000000000000000000112").unwrap();

			assert_ok!(LiquidationEvmBridge::<Runtime>::liquidate(
				InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: alice_evm_addr(),
				},
				collateral,
				repay_dest,
				100,
				100,
			));
			System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Executed {
				from: Default::default(),
				contract: erc20_address(),
				logs: vec![module_evm::Log {
					address: erc20_address(),
					topics: vec![
						H256::from_str("0xf3fa0eaee8f258c23b013654df25d1527f98a5c7ccd5e951dd77caca400ef972").unwrap(),
					],
					data: {
						let mut buf = [0u8; 128];
						buf[12..32].copy_from_slice(collateral.as_bytes());
						buf[44..64].copy_from_slice(repay_dest.as_bytes());
						let mut amount_data = [0u8; 32];
						U256::from(100).to_big_endian(&mut amount_data);
						buf[64..96].copy_from_slice(&amount_data);
						buf[96..128].copy_from_slice(&amount_data);
						buf.to_vec()
					},
				}],
				used_gas: 25061,
				used_storage: 0,
			}));
		});
}

#[test]
fn on_collateral_transfer_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_liquidation_ok_contracts();
			let collateral = EvmAddress::from_str("1000000000000000000000000000000000000111").unwrap();
			LiquidationEvmBridge::<Runtime>::on_collateral_transfer(
				InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: alice_evm_addr(),
				},
				collateral,
				100,
			);
			System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Executed {
				from: Default::default(),
				contract: erc20_address(),
				logs: vec![module_evm::Log {
					address: erc20_address(),
					topics: vec![
						H256::from_str("0xa5625c5568ddba471a5e1190863744239495ca35883ce7f3e7d3beea2e89be74").unwrap(),
					],
					data: {
						let mut buf = [0u8; 64];
						buf[12..32].copy_from_slice(collateral.as_bytes());
						let mut amount_data = [0u8; 32];
						U256::from(100).to_big_endian(&mut amount_data);
						buf[32..64].copy_from_slice(&amount_data);
						buf.to_vec()
					},
				}],
				used_gas: 23560,
				used_storage: 0,
			}));
		});
}

#[test]
fn on_repayment_refund_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_liquidation_ok_contracts();
			let collateral = EvmAddress::from_str("1000000000000000000000000000000000000111").unwrap();
			LiquidationEvmBridge::<Runtime>::on_repayment_refund(
				InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: alice_evm_addr(),
				},
				collateral,
				100,
			);
			System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Executed {
				from: Default::default(),
				contract: erc20_address(),
				logs: vec![module_evm::Log {
					address: erc20_address(),
					topics: vec![
						H256::from_str("0x003d5a25faf4a774379f05de4f94d8967080f7e731902eb8f542b957a0712e18").unwrap(),
					],
					data: {
						let mut buf = [0u8; 64];
						buf[12..32].copy_from_slice(collateral.as_bytes());
						let mut amount_data = [0u8; 32];
						U256::from(100).to_big_endian(&mut amount_data);
						buf[32..64].copy_from_slice(&amount_data);
						buf.to_vec()
					},
				}],
				used_gas: 23582,
				used_storage: 0,
			}));
		});
}

#[test]
fn liquidation_err_fails_as_expected() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_liquidation_err_contracts();
			let collateral = EvmAddress::from_str("1000000000000000000000000000000000000111").unwrap();
			let repay_dest = EvmAddress::from_str("1000000000000000000000000000000000000112").unwrap();

			assert_noop!(
				LiquidationEvmBridge::<Runtime>::liquidate(
					InvokeContext {
						contract: erc20_address(),
						sender: Default::default(),
						origin: alice_evm_addr(),
					},
					collateral,
					repay_dest,
					100,
					100,
				),
				Error::<Runtime>::ExecutionRevert,
			);
		});
}

#[cfg(feature = "tracing")]
#[test]
fn tracing_should_work() {
	use module_evm::runner::tracing;
	use primitives::evm::tracing::TracerConfig;

	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000), (bob(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let mut tracer = tracing::Tracer::new(TracerConfig::CallTracer);
			tracing::using(&mut tracer, || {
				assert_err!(
					EVMBridge::<Runtime>::transfer(
						InvokeContext {
							contract: erc20_address(),
							sender: bob_evm_addr(),
							origin: bob_evm_addr(),
						},
						alice_evm_addr(),
						10
					),
					Error::<Runtime>::ExecutionRevert
				);
			});
			assert_json_snapshot!(tracer.finalize(), @r###"
   {
     "Calls": [
       {
         "type": "CALL",
         "from": "0x1000000000000000000000000000000000000002",
         "to": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
         "input": "0xa9059cbb0000000000000000000000001000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000a",
         "value": "0x0",
         "gas": 200000,
         "gasUsed": 200000,
         "output": null,
         "error": null,
         "revertReason": "0xe450d38c00000000000000000000000010000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a",
         "depth": 0,
         "logs": [
           {
             "sLoad": {
               "address": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
               "index": "0xfb750de6f7d0583f749efc558ce6626b24fed04efd7219dc3f4294c408699e8c",
               "value": "0x0000000000000000000000000000000000000000000000000000000000000000"
             }
           }
         ],
         "calls": []
       }
     ]
   }
   "###);

			tracing::using(&mut tracer, || {
				assert_ok!(EVMBridge::<Runtime>::transfer(
					InvokeContext {
						contract: erc20_address(),
						sender: alice_evm_addr(),
						origin: alice_evm_addr(),
					},
					bob_evm_addr(),
					100
				));
			});

			assert_json_snapshot!(tracer.finalize(), @r###"
   {
     "Calls": [
       {
         "type": "CALL",
         "from": "0x1000000000000000000000000000000000000001",
         "to": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
         "input": "0xa9059cbb00000000000000000000000010000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000064",
         "value": "0x0",
         "gas": 200000,
         "gasUsed": 51906,
         "output": "0x0000000000000000000000000000000000000000000000000000000000000001",
         "error": null,
         "revertReason": null,
         "depth": 0,
         "logs": [
           {
             "sLoad": {
               "address": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
               "index": "0xe6f18b3f6d2cdeb50fb82c61f7a7a249abf7b534575880ddcfde84bba07ce81d",
               "value": "0x00000000000000000000000000000000000000000000152d02c7e14af6800000"
             }
           },
           {
             "sStore": {
               "address": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
               "index": "0xe6f18b3f6d2cdeb50fb82c61f7a7a249abf7b534575880ddcfde84bba07ce81d",
               "value": "0x00000000000000000000000000000000000000000000152d02c7e14af67fff9c"
             }
           },
           {
             "sLoad": {
               "address": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
               "index": "0xfb750de6f7d0583f749efc558ce6626b24fed04efd7219dc3f4294c408699e8c",
               "value": "0x0000000000000000000000000000000000000000000000000000000000000000"
             }
           },
           {
             "sStore": {
               "address": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
               "index": "0xfb750de6f7d0583f749efc558ce6626b24fed04efd7219dc3f4294c408699e8c",
               "value": "0x0000000000000000000000000000000000000000000000000000000000000064"
             }
           },
           {
             "log": {
               "address": "0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643",
               "topics": [
                 "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                 "0x0000000000000000000000001000000000000000000000000000000000000001",
                 "0x0000000000000000000000001000000000000000000000000000000000000002"
               ],
               "data": "0x0000000000000000000000000000000000000000000000000000000000000064"
             }
           }
         ],
         "calls": []
       }
     ]
   }
   "###);
		});
}
