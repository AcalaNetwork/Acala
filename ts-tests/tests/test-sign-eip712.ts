import { expect, beforeAll, it } from "vitest";
import { describeWithAcala, getEvmNonce, transfer } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import { Wallet } from "@ethersproject/wallet";
import { encodeAddress } from "@polkadot/keyring";
import { hexToU8a, u8aConcat, stringToU8a } from "@polkadot/util";
import { ethers, BigNumber, ContractFactory } from "ethers";
import Erc20DemoContract from "../build/Erc20DemoContract.json"

describeWithAcala("Acala RPC (Sign eip712)", (context) => {
	let alice: BodhiSigner;
	let signer: Wallet;
	let subAddr: string;
	let factory: ContractFactory;
	let contract: string;

	beforeAll(async function () {
		[alice] = context.wallets;

		signer = new Wallet(
			"0x0123456789012345678901234567890123456789012345678901234567890123"
		);

		subAddr = encodeAddress(
			u8aConcat(
				stringToU8a("evm:"),
				hexToU8a(signer.address),
				new Uint8Array(8).fill(0)
			)
		);

		expect(subAddr).to.equal("5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc");

		await transfer(context, alice.substrateAddress, subAddr, 10000000000000);

		factory = new ethers.ContractFactory(Erc20DemoContract.abi, Erc20DemoContract.bytecode);
	});

	it("create should sign and verify", async function () {
		const domain = {
			name: "Acala EVM",
			version: "1",
			chainId: +context.provider.api.consts.evmAccounts.chainId.toString(),
			salt: (await context.provider.api.rpc.chain.getBlockHash(0)).toHex(),
		};

		const nonce = await getEvmNonce(context.provider, signer.address);

		const types = {
			AccessList: [
				{ name: "address", type: "address" },
				{ name: "storageKeys", type: "uint256[]" },
			],
			Transaction: [
				{ name: "action", type: "string" },
				{ name: "to", type: "address" },
				{ name: "nonce", type: "uint256" },
				{ name: "tip", type: "uint256" },
				{ name: "data", type: "bytes" },
				{ name: "value", type: "uint256" },
				{ name: "gasLimit", type: "uint256" },
				{ name: "storageLimit", type: "uint256" },
				{ name: "accessList", type: "AccessList[]" },
				{ name: "validUntil", type: "uint256" },
			],
		};

		const deploy = factory.getDeployTransaction(100000);

		// The data to sign
		const value = {
			action: "Create",
			to: "0x0000000000000000000000000000000000000000",
			nonce: nonce,
			tip: 2,
			data: deploy.data,
			value: '0',
			gasLimit: 2100000,
			storageLimit: 20000,
			accessList: [],
			//accessList: [
			//	{
			//		address: "0x0000000000000000000000000000000000000000",
			//		storageKeys: [
			//			"0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
			//			"0x0000000000111111111122222222223333333333444444444455555555556666",
			//			"0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
			//		]
			//	}
			//],
			validUntil: (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100,
		};

		const tx = context.provider.api.tx.evm.ethCall(
			{ Create: null },
			value.data as any,
			value.value,
			value.gasLimit,
			value.storageLimit,
			value.accessList,
			value.validUntil,
		);

		const signature = await signer._signTypedData(domain, types, value);
		const sig = context.provider.api.createType("ExtrinsicSignature", { AcalaEip712: signature }).toHex()

		tx.addSignature(subAddr, { AcalaEip712: signature } as any, {
			blockHash: domain.salt, // ignored
			era: "0x00", // mortal
			genesisHash: domain.salt, // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: value.tip,
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
					"signer": {
						"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
					},
					"signature": {
						"acalaEip712": "${signature}"
					},
					"era": {
						"immortalEra": "0x00"
					},
					"nonce": 0,
					"tip": 2
				},
				"method": {
					"callIndex": "0xb400",
					"args": {
						"action": {
							"create": null
						},
						"input": "${deploy.data}",
						"value": 0,
						"gas_limit": 2100000,
						"storage_limit": 20000,
						"access_list": [],
						"valid_until": 106
					}
				}
			  }`.toString().replace(/\s/g, '')
		);

		await new Promise(async (resolve) => {
			tx.send((result) => {
				if (result.status.isFinalized || result.status.isInBlock) {
					resolve(undefined);
				}
			});
		});

		let current_block_number = (await context.provider.api.query.system.number()).toNumber();
		let block_hash = await context.provider.api.rpc.chain.getBlockHash(current_block_number);
		const result = await context.provider.api.derive.tx.events(block_hash);
		// console.log("current_block_number: ", current_block_number, " event: ", result.events.toString());

		let event = result.events.filter(item => context.provider.api.events.evm.Created.is(item.event));
		expect(event.length).to.equal(1);
		// console.log(event[0].toString())

		// get address
		contract = event[0].event.data[1].toString();
	});

	it("call should sign and verify", async function () {
		const domain = {
			name: "Acala EVM",
			version: "1",
			chainId: +context.provider.api.consts.evmAccounts.chainId.toString(),
			salt: (await context.provider.api.rpc.chain.getBlockHash(0)).toHex(),
		};

		const nonce = await getEvmNonce(context.provider, signer.address);

		const types = {
			AccessList: [
				{ name: "address", type: "address" },
				{ name: "storageKeys", type: "uint256[]" },
			],
			Transaction: [
				{ name: "action", type: "string" },
				{ name: "to", type: "address" },
				{ name: "nonce", type: "uint256" },
				{ name: "tip", type: "uint256" },
				{ name: "data", type: "bytes" },
				{ name: "value", type: "uint256" },
				{ name: "gasLimit", type: "uint256" },
				{ name: "storageLimit", type: "uint256" },
				{ name: "accessList", type: "AccessList[]" },
				{ name: "validUntil", type: "uint256" },
			],
		};

		const receiver = '0x1111222233334444555566667777888899990000';
		const input = await factory.attach(contract).populateTransaction.transfer(receiver, 100);

		// The data to sign
		const value = {
			action: "Call",
			to: contract,
			nonce: nonce,
			tip: 2,
			data: input.data,
			value: '0',
			gasLimit: 210000,
			storageLimit: 1000,
			accessList: [],
			validUntil: (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100,
		};

		const tx = context.provider.api.tx.evm.ethCall(
			{ Call: value.to },
			value.data as any,
			value.value,
			value.gasLimit,
			value.storageLimit,
			value.accessList,
			value.validUntil
		);

		const signature = await signer._signTypedData(domain, types, value);
		const sig = context.provider.api.createType("ExtrinsicSignature", { AcalaEip712: signature }).toHex()

		tx.addSignature(subAddr, { AcalaEip712: signature } as any, {
			blockHash: domain.salt, // ignored
			era: "0x00", // mortal
			genesisHash: domain.salt, // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: value.tip,
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
					"signer": {
						"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
					},
					"signature": {
						"acalaEip712": "${signature}"
					},
					"era": {
						"immortalEra": "0x00"
					},
					"nonce": 1,
					"tip": 2
				},
				"method": {
					"callIndex": "0xb400",
					"args": {
						"action": {
							"call": "${contract}"
						},
						"input": "${input.data}",
						"value": 0,
						"gas_limit": 210000,
						"storage_limit": 1000,
						"access_list": [],
						"valid_until": 107
					}
				}
			  }`.toString().replace(/\s/g, '')
		);

		await new Promise(async (resolve) => {
			tx.send((result) => {
				if (result.status.isFinalized || result.status.isInBlock) {
					resolve(undefined);
				}
			});
		});

		await new Promise(async (resolve) => {
			context.provider.api.tx.sudo.sudo(context.provider.api.tx.evm.publishFree(contract)).signAndSend(alice.substrateAddress, ((result) => {
				if (result.status.isFinalized || result.status.isInBlock) {
					resolve(undefined);
				}
			}));
		});

		const erc20 = new ethers.Contract(contract, Erc20DemoContract.abi, alice);
		expect((await erc20.balanceOf(signer.address)).toString()).to.equal("99900");
		expect((await erc20.balanceOf(receiver)).toString()).to.equal("100");
	});
});
