import { expect } from "chai";

import { describeWithAcala, nextBlock } from "./util";
import { Signer } from "@acala-network/bodhi";
import { Wallet } from "@ethersproject/wallet";
import { encodeAddress } from "@polkadot/keyring";
import { hexToU8a, u8aConcat, stringToU8a } from "@polkadot/util";
import { ethers, BigNumber, ContractFactory } from "ethers";
import Erc20DemoContract from "../build/Erc20DemoContract.json"

describeWithAcala("Acala RPC (Sign eth)", (context) => {
	let alice: Signer;
	let signer: Wallet;
	let subAddr: string;
	let factory: ContractFactory;
	let contract: string;

	before("init", async function () {
		this.timeout(15000);
		[alice] = await context.provider.getWallets();

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

		await context.provider.api.tx.balances.transfer(subAddr, "10_000_000_000_000")
			.signAndSend(await alice.getSubstrateAddress());

		factory = new ethers.ContractFactory(Erc20DemoContract.abi, Erc20DemoContract.bytecode);
	});

	it("create should sign and verify", async function () {
		this.timeout(150000);

		const chanid = +context.provider.api.consts.evm.chainId.toString()
		const nonce = (await context.provider.api.query.system.account(subAddr)).nonce.toNumber()
		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100
		const storageLimit = 20000

		const gasPrice = '0x' + (BigInt(storageLimit) << BigInt(32) | BigInt(validUntil)).toString(16);

		const deploy = factory.getDeployTransaction(100000);

		const value = {
			// to: "0x0000000000000000000000000000000000000000",
			nonce,
			gasLimit: 2100000,
			gasPrice,
			data: deploy.data,
			value: 0,
			chainId: chanid,
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			nonce: 0,
			gasPrice: BigNumber.from('0x4e2000000069'),
			gasLimit: BigNumber.from(2100000),
			// to: '0x0000000000000000000000000000000000000000',
			value: BigNumber.from(0),
			data: deploy.data,
			chainId: 595,
			// v: 1226,
			// r: '0xff8ff25480f5e1d1b38603b8fa1f10d64faf81707768dd9016fc4dd86d5474d2',
			// s: '0x6c2cfd5acd5b0b820e1c107efd5e7ce2c452b81742091f43f5c793a835c8644f',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x456d37c868520b362bbf5baf1b19752818eba49cc92c1a512e2e80d1ccfbc18b',
			type: null
		});

		const tx = context.provider.api.tx.evm.ethCall(
			{ Create: null },
			value.data,
			value.value,
			value.gasLimit,
			storageLimit,
			validUntil
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
				  "callIndex": "0xb400",
				  "args": {
					"action": {
					  "create": null
					},
					"input": "${deploy.data}",
					"value": 0,
					"gas_limit": 2100000,
					"storage_limit": 20000,
					"valid_until": 105
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
		this.timeout(150000);

		const chanid = +context.provider.api.consts.evm.chainId.toString()
		const nonce = (await context.provider.api.query.system.account(subAddr)).nonce.toNumber()
		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100
		const storageLimit = 1000

		const gasPrice = '0x' + (BigInt(storageLimit) << BigInt(32) | BigInt(validUntil)).toString(16);
		const receiver = '0x1111222233334444555566667777888899990000';
		const input = await factory.attach(contract).populateTransaction.transfer(receiver, 100);

		const value = {
			to: contract,
			nonce,
			gasLimit: 210000,
			gasPrice,
			data: input.data,
			value: 0,
			chainId: chanid,
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			nonce: 1,
			gasPrice: BigNumber.from('0x03e80000006a'),
			gasLimit: BigNumber.from(210000),
			to: ethers.utils.getAddress(contract),
			value: BigNumber.from(0),
			data: input.data,
			chainId: 595,
			// v: 1225,
			// r: '0xf84345a6459785986a1b2df711fe02597d70c1393757a243f8f924ea541d2ecb',
			// s: '0x51476de1aa437cd820d59e1d9836e37e643fec711fe419464e637cab59291875',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x67274cd0347795d0e2986021a19b1347948a0a93e1fb31a315048320fbfcae8a',
			type: null
		});

		const tx = context.provider.api.tx.evm.ethCall(
			{ Call: value.to },
			value.data,
			value.value,
			value.gasLimit,
			storageLimit,
			validUntil
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
				  "callIndex": "0xb400",
				  "args": {
					"action": {
					  "call": "${contract}"
					},
					"input": "${input.data}",
					"value": 0,
					"gas_limit": 210000,
					"storage_limit": 1000,
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

		await new Promise(async (resolve) => {
			context.provider.api.tx.sudo.sudo(context.provider.api.tx.evm.deployFree(contract)).signAndSend(await alice.getSubstrateAddress(), ((result) => {
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
