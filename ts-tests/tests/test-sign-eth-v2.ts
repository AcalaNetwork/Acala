import { expect, beforeAll, it } from "vitest";

import { describeWithAcala, getEvmNonce, transfer } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import { Wallet } from "@ethersproject/wallet";
import { encodeAddress } from "@polkadot/keyring";
import { hexToU8a, u8aConcat, stringToU8a } from "@polkadot/util";
import { ethers, BigNumber, ContractFactory } from "ethers";
import Erc20DemoContract from "../build/Erc20DemoContract.json"

// const GAS_MASK = 100000;
const STORAGE_MASK = 100;
const GAS_LIMIT_CHUNK = BigNumber.from(30000);
const TEN_GWEI = BigNumber.from(10000000000);

describeWithAcala("Acala RPC (Sign eth)", (context) => {
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
		const chainId = +context.provider.api.consts.evmAccounts.chainId.toString()
		const nonce = await getEvmNonce(context.provider, signer.address);

		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100
		const storageLimit = 20000;
		const gasLimit = BigNumber.from('2100000');

		// 100 Gwei
		const txFeePerGas = BigNumber.from('100000000000');
		const txGasPrice = txFeePerGas.add(validUntil);
		const encodedGasLimit = gasLimit.div(GAS_LIMIT_CHUNK).add(1);
		const encodedStorageLimit = Math.ceil(Math.log2(storageLimit));
		// ignore the tx fee
		const txGasLimit = encodedGasLimit.mul(STORAGE_MASK).add(encodedStorageLimit);

		const deploy = factory.getDeployTransaction(100000);

		const value = {
			// to: "0x0000000000000000000000000000000000000000",
			nonce: nonce,
			gasLimit: txGasLimit.toNumber(),
			gasPrice: txGasPrice.toHexString(),
			data: deploy.data,
			value: 0,
			chainId: chainId,
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			nonce: 0,
			// gasPrice: BigNumber.from(100000000105),
			// gasLimit: BigNumber.from(7115),
			// to: '0x0000000000000000000000000000000000000000',
			// value: BigNumber.from(0),
			data: deploy.data,
			chainId: 595,
			// v: 1226,
			// r: '0xff8ff25480f5e1d1b38603b8fa1f10d64faf81707768dd9016fc4dd86d5474d2',
			// s: '0x6c2cfd5acd5b0b820e1c107efd5e7ce2c452b81742091f43f5c793a835c8644f',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x456d37c868520b362bbf5baf1b19752818eba49cc92c1a512e2e80d1ccfbc18b',
			type: null
		});
		expect(rawtx.gasPrice?.toNumber()).toMatchInlineSnapshot(`100000000106`)
		expect(rawtx.gasLimit?.toNumber()).toMatchInlineSnapshot(`7115`)
		expect(rawtx.value?.toNumber()).to.eq(0);

		const tx = context.provider.api.tx.evm.ethCallV2(
			{ Create: null },
			value.data as any,
			value.value,
			txGasPrice.toNumber(),
			txGasLimit.toNumber(),
			[], // accessList
		);

		const sig = ethers.utils.joinSignature({ r: rawtx.r!, s: rawtx.s, v: rawtx.v })

		tx.addSignature(subAddr, { Ethereum: sig } as any, {
			blockHash: '0x', // ignored
			era: "0x00", // mortal
			genesisHash: '0x', // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: 0,
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
				  "signer": {
					"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
				  },
				  "signature": {
					"ethereum": "${sig}"
				  },
				  "era": {
					"immortalEra": "0x00"
				  },
				  "nonce": 0,
				  "tip": 0
				},
				"method": {
				  "callIndex": "0xb40f",
				  "args": {
					"action": {
					  "create": null
					},
					"input": "${deploy.data}",
					"value": 0,
					"gas_price": ${rawtx.gasPrice},
					"gas_limit": ${rawtx.gasLimit},
					"access_list": []
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
		const chainId = +context.provider.api.consts.evmAccounts.chainId.toString();
		const nonce = await getEvmNonce(context.provider, signer.address);

		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100;
		const storageLimit = 1000;
		const gasLimit = BigNumber.from('210000');

		// 100 Gwei
		const txFeePerGas = BigNumber.from('100000000000');
		const txGasPrice = txFeePerGas.add(validUntil);
		const encodedGasLimit = gasLimit.div(GAS_LIMIT_CHUNK).add(1);
		const encodedStorageLimit = Math.ceil(Math.log2(storageLimit));
		// ignore the tx fee
		const txGasLimit = encodedGasLimit.mul(STORAGE_MASK).add(encodedStorageLimit);
		const receiver = '0x1111222233334444555566667777888899990000';
		const input = await factory.attach(contract).populateTransaction.transfer(receiver, 100);

		const value = {
			to: contract,
			nonce: nonce,
			gasLimit: txGasLimit.toNumber(),
			gasPrice: txGasPrice.toHexString(),
			data: input.data,
			value: 0,
			chainId: chainId,
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			nonce: 1,
			// gasPrice: BigNumber.from(100000000106),
			// gasLimit: BigNumber.from(810),
			to: ethers.utils.getAddress(contract),
			// value: BigNumber.from(0),
			data: input.data,
			chainId: 595,
			// v: 1225,
			// r: '0xf84345a6459785986a1b2df711fe02597d70c1393757a243f8f924ea541d2ecb',
			// s: '0x51476de1aa437cd820d59e1d9836e37e643fec711fe419464e637cab59291875',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x67274cd0347795d0e2986021a19b1347948a0a93e1fb31a315048320fbfcae8a',
			type: null
		});
		expect(rawtx.gasPrice?.toNumber()).toMatchInlineSnapshot(`100000000107`)
		expect(rawtx.gasLimit?.toNumber()).toMatchInlineSnapshot(`810`)
		expect(rawtx.value?.toNumber()).to.eq(0);

		const tx = context.provider.api.tx.evm.ethCallV2(
			{ Call: value.to },
			value.data as any,
			value.value,
			txGasPrice.toNumber(),
			txGasLimit.toNumber(),
			[], // accessList
		);

		const sig = ethers.utils.joinSignature({ r: rawtx.r!, s: rawtx.s, v: rawtx.v })

		tx.addSignature(subAddr, { Ethereum: sig } as any, {
			blockHash: '0x', // ignored
			era: "0x00", // mortal
			genesisHash: '0x', // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: 0,
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
				  "signer": {
					"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
				  },
				  "signature": {
					"ethereum": "${sig}"
				  },
				  "era": {
					"immortalEra": "0x00"
				  },
				  "nonce": 1,
				  "tip": 0
				},
				"method": {
				  "callIndex": "0xb40f",
				  "args": {
					"action": {
					  "call": "${contract}"
					},
					"input": "${input.data}",
					"value": 0,
					"gas_price": ${rawtx.gasPrice},
					"gas_limit": ${rawtx.gasLimit},
					"access_list": []
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

describeWithAcala("Acala RPC (Sign eth with tip)", (context) => {
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
		const chainId = +context.provider.api.consts.evmAccounts.chainId.toString()
		const nonce = await getEvmNonce(context.provider, signer.address);

		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100
		const storageLimit = 20000;
		const gasLimit = BigNumber.from('2100000');

		// 10%
		const tipNumber = BigNumber.from('1');
		// 100 Gwei
		const txFeePerGas = BigNumber.from('110000000000');
		const txGasPrice = txFeePerGas.add(validUntil);
		const encodedGasLimit = gasLimit.div(GAS_LIMIT_CHUNK).add(1);
		const encodedStorageLimit = Math.ceil(Math.log2(storageLimit));
		// tx fee = 100_00000
		const txGasLimit = BigNumber.from('10000000').add(encodedGasLimit.mul(STORAGE_MASK)).add(encodedStorageLimit);
		const tip = txGasPrice.sub(tipNumber.mul(TEN_GWEI)).mul(txGasLimit).mul(tipNumber).div(10).div(1000000);

		const deploy = factory.getDeployTransaction(100000);

		const value = {
			// to: "0x0000000000000000000000000000000000000000",
			nonce: nonce,
			gasLimit: txGasLimit.toNumber(),
			gasPrice: txGasPrice.toHexString(),
			data: deploy.data,
			value: 0,
			chainId: chainId,
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			nonce: 0,
			// gasPrice: BigNumber.from(110000000105),
			// gasLimit: BigNumber.from(10007115),
			// to: '0x0000000000000000000000000000000000000000',
			// value: BigNumber.from(0),
			data: deploy.data,
			chainId: 595,
			// v: 1226,
			// r: '0xff8ff25480f5e1d1b38603b8fa1f10d64faf81707768dd9016fc4dd86d5474d2',
			// s: '0x6c2cfd5acd5b0b820e1c107efd5e7ce2c452b81742091f43f5c793a835c8644f',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x456d37c868520b362bbf5baf1b19752818eba49cc92c1a512e2e80d1ccfbc18b',
			type: null
		});
		expect(rawtx.gasPrice?.toNumber()).toMatchInlineSnapshot(`110000000106`)
		expect(rawtx.gasLimit?.toNumber()).toMatchInlineSnapshot(`10007115`)
		expect(rawtx.value?.toNumber()).to.eq(0);

		const tx = context.provider.api.tx.evm.ethCallV2(
			{ Create: null },
			value.data as any,
			value.value,
			txGasPrice.toNumber(),
			txGasLimit.toNumber(),
			[], // accessList
		);

		const sig = ethers.utils.joinSignature({ r: rawtx.r!, s: rawtx.s, v: rawtx.v })

		tx.addSignature(subAddr, { Ethereum: sig } as any, {
			blockHash: '0x', // ignored
			era: "0x00", // mortal
			genesisHash: '0x', // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: tip.toString(),
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
				  "signer": {
					"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
				  },
				  "signature": {
					"ethereum": "${sig}"
				  },
				  "era": {
					"immortalEra": "0x00"
				  },
				  "nonce": 0,
				  "tip": ${tip}
				},
				"method": {
				  "callIndex": "0xb40f",
				  "args": {
					"action": {
					  "create": null
					},
					"input": "${deploy.data}",
					"value": 0,
					"gas_price": ${rawtx.gasPrice},
					"gas_limit": ${rawtx.gasLimit},
					"access_list": []
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
		const chainId = +context.provider.api.consts.evmAccounts.chainId.toString();
		const nonce = await getEvmNonce(context.provider, signer.address);

		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100;
		const storageLimit = 1000;
		const gasLimit = BigNumber.from('210000');

		// 10%
		const tipNumber = BigNumber.from('1');
		// 100 Gwei
		const txFeePerGas = BigNumber.from('110000000000');
		const txGasPrice = txFeePerGas.add(validUntil);
		const encodedGasLimit = gasLimit.div(GAS_LIMIT_CHUNK).add(1);
		const encodedStorageLimit = Math.ceil(Math.log2(storageLimit));
		// tx fee = 100_00000
		const txGasLimit = BigNumber.from('10000000').add(encodedGasLimit.mul(STORAGE_MASK)).add(encodedStorageLimit);
		const tip = txGasPrice.sub(tipNumber.mul(TEN_GWEI)).mul(txGasLimit).mul(tipNumber).div(10).div(1000000);

		const receiver = '0x1111222233334444555566667777888899990000';
		const input = await factory.attach(contract).populateTransaction.transfer(receiver, 100);

		const value = {
			to: contract,
			nonce: nonce,
			gasLimit: txGasLimit.toNumber(),
			gasPrice: txGasPrice.toHexString(),
			data: input.data,
			value: 0,
			chainId: chainId,
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			nonce: 1,
			// gasPrice: BigNumber.from(110000000106),
			// gasLimit: BigNumber.from(10000810),
			to: ethers.utils.getAddress(contract),
			// value: BigNumber.from(0),
			data: input.data,
			chainId: 595,
			// v: 1225,
			// r: '0xf84345a6459785986a1b2df711fe02597d70c1393757a243f8f924ea541d2ecb',
			// s: '0x51476de1aa437cd820d59e1d9836e37e643fec711fe419464e637cab59291875',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x67274cd0347795d0e2986021a19b1347948a0a93e1fb31a315048320fbfcae8a',
			type: null
		});
		expect(rawtx.gasPrice?.toNumber()).toMatchInlineSnapshot(`110000000107`)
		expect(rawtx.gasLimit?.toNumber()).toMatchInlineSnapshot(`10000810`)
		expect(rawtx.value?.toNumber()).to.eq(0);

		const tx = context.provider.api.tx.evm.ethCallV2(
			{ Call: value.to },
			value.data as any,
			value.value,
			txGasPrice.toNumber(),
			txGasLimit.toNumber(),
			[], // accessList
		);

		const sig = ethers.utils.joinSignature({ r: rawtx.r!, s: rawtx.s, v: rawtx.v })

		tx.addSignature(subAddr, { Ethereum: sig } as any, {
			blockHash: '0x', // ignored
			era: "0x00", // mortal
			genesisHash: '0x', // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: tip.toString(),
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
				  "signer": {
					"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
				  },
				  "signature": {
					"ethereum": "${sig}"
				  },
				  "era": {
					"immortalEra": "0x00"
				  },
				  "nonce": 1,
				  "tip": ${tip}
				},
				"method": {
				  "callIndex": "0xb40f",
				  "args": {
					"action": {
					  "call": "${contract}"
					},
					"input": "${input.data}",
					"value": 0,
					"gas_price": ${rawtx.gasPrice},
					"gas_limit": ${rawtx.gasLimit},
					"access_list": []
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
