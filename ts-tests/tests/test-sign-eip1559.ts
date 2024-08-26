import { expect, beforeAll, it } from "vitest";

import { describeWithAcala, getEvmNonce, transfer } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import { Wallet } from "@ethersproject/wallet";
import { encodeAddress } from "@polkadot/keyring";
import { hexToU8a, u8aConcat, stringToU8a } from "@polkadot/util";
import { ethers, BigNumber, ContractFactory } from "ethers";
import Erc20DemoContract from "../build/Erc20DemoContract.json"

describeWithAcala("Acala RPC (Sign eip1559)", (context) => {
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

	const bigNumDiv = (x: BigNumber, y: BigNumber) => {
		const res = x.div(y);
		return res.mul(y) === x
			? res
			: res.add(1)
	}

	it("create should sign and verify", async function () {
		const chain_id = +context.provider.api.consts.evmAccounts.chainId.toString()
		const nonce = await getEvmNonce(context.provider, signer.address);
		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100
		const storageLimit = 20000;
		const gasLimit = 2100000;
		const priorityFee = BigNumber.from(2);
		const tip = priorityFee.mul(gasLimit).toNumber();

		const block_period = bigNumDiv(BigNumber.from(validUntil), BigNumber.from(30));
		const storage_entry_limit = bigNumDiv(BigNumber.from(storageLimit), BigNumber.from(64));
		const storage_byte_deposit = BigNumber.from(context.provider.api.consts.evm.storageDepositPerByte.toString());
		const storage_entry_deposit = storage_byte_deposit.mul(64);
		const tx_fee_per_gas = BigNumber.from(context.provider.api.consts.evm.txFeePerGas.toString());
		const tx_gas_price = tx_fee_per_gas.add(block_period.toNumber() << 16).add(storage_entry_limit);
		// There is a loss of precision here, so the order of calculation must be guaranteed
		// must ensure storage_deposit / tx_fee_per_gas * storage_limit
		const tx_gas_limit = storage_entry_deposit.div(tx_fee_per_gas).mul(storage_entry_limit).add(gasLimit);

		const deploy = factory.getDeployTransaction(100000);

		const value = {
			type: 2, // EIP-1559
			// to: "0x0000000000000000000000000000000000000000",
			nonce: nonce,
			gasLimit: tx_gas_limit.toNumber(),
			data: deploy.data,
			value: 0,
			chainId: chain_id,
			maxPriorityFeePerGas: priorityFee.toHexString(),
			maxFeePerGas: tx_gas_price.toHexString(),
			accessList: [],
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			type: 2,
			chainId: 595,
			nonce: 0,
			// maxPriorityFeePerGas: BigNumber.from(2),
			// maxFeePerGas: BigNumber.from(200000209209),
			gasPrice: null,
			// gasLimit: BigNumber.from(12116000),
			to: null,
			// value: BigNumber.from(0),
			data: deploy.data,
			accessList: [],
			// v: 1226,
			// r: '0xff8ff25480f5e1d1b38603b8fa1f10d64faf81707768dd9016fc4dd86d5474d2',
			// s: '0x6c2cfd5acd5b0b820e1c107efd5e7ce2c452b81742091f43f5c793a835c8644f',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x456d37c868520b362bbf5baf1b19752818eba49cc92c1a512e2e80d1ccfbc18b',
		});
		expect(rawtx.maxPriorityFeePerGas?.toNumber()).to.equal(2);
		expect(rawtx.maxFeePerGas?.toNumber()).to.equal(200000209209);
		expect(rawtx.gasLimit?.toNumber()).to.equal(12116000);
		expect(rawtx.value?.toNumber()).to.equal(0);

		// tx data to user input
		const input_storage_entry_limit = tx_gas_price.and(0xffff);
		const input_storage_limit = input_storage_entry_limit.mul(64);
		const input_block_period = (tx_gas_price.sub(input_storage_entry_limit).sub(tx_fee_per_gas).toNumber()) >> 16;
		const input_valid_until = input_block_period * 30;
		const input_gas_limit = tx_gas_limit.sub(storage_entry_deposit.div(tx_fee_per_gas).mul(input_storage_entry_limit));

		const tx = context.provider.api.tx.evm.ethCall(
			{ Create: null },
			value.data as any,
			value.value,
			input_gas_limit.toNumber(),
			input_storage_limit.toNumber(),
			value.accessList,
			input_valid_until
		);

		const sig = ethers.utils.joinSignature({ r: rawtx.r!, s: rawtx.s, v: rawtx.v })

		tx.addSignature(subAddr, { Eip1559: sig } as any, {
			blockHash: '0x', // ignored
			era: "0x00", // mortal
			genesisHash: '0x', // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: tip,
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
				  "signer": {
					"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
				  },
				  "signature": {
					"eip1559": "${sig}"
				  },
				  "era": {
					"immortalEra": "0x00"
				  },
				  "nonce": 0,
				  "tip": ${tip}
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
					"storage_limit": 20032,
					"access_list": [],
					"valid_until": 120
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
		const chain_id = +context.provider.api.consts.evmAccounts.chainId.toString();
		const nonce = await getEvmNonce(context.provider, signer.address);
		const validUntil = (await context.provider.api.rpc.chain.getHeader()).number.toNumber() + 100;
		const storageLimit = 1000;
		const gasLimit = 210000;
		const priorityFee = BigNumber.from(2);
		const tip = priorityFee.mul(gasLimit).toNumber();

		const block_period = bigNumDiv(BigNumber.from(validUntil), BigNumber.from(30));
		const storage_entry_limit = bigNumDiv(BigNumber.from(storageLimit), BigNumber.from(64));
		const storage_byte_deposit = BigNumber.from(context.provider.api.consts.evm.storageDepositPerByte.toString());
		const storage_entry_deposit = storage_byte_deposit.mul(64);
		const tx_fee_per_gas = BigNumber.from(context.provider.api.consts.evm.txFeePerGas.toString());
		const tx_gas_price = tx_fee_per_gas.add(block_period.toNumber() << 16).add(storage_entry_limit);
		// There is a loss of precision here, so the order of calculation must be guaranteed
		// must ensure storage_deposit / tx_fee_per_gas * storage_limit
		const tx_gas_limit = storage_entry_deposit.div(tx_fee_per_gas).mul(storage_entry_limit).add(gasLimit);

		const receiver = '0x1111222233334444555566667777888899990000';
		const input = await factory.attach(contract).populateTransaction.transfer(receiver, 100);

		const value = {
			type: 2, // EIP-1559
			to: contract,
			nonce: nonce,
			gasLimit: tx_gas_limit.toNumber(),
			data: input.data,
			value: 0,
			chainId: chain_id,
			maxPriorityFeePerGas: priorityFee.toHexString(),
			maxFeePerGas: tx_gas_price.toHexString(),
			accessList: [],
		}

		const signedTx = await signer.signTransaction(value)
		const rawtx = ethers.utils.parseTransaction(signedTx)

		expect(rawtx).to.deep.include({
			type: 2,
			chainId: 595,
			nonce: 1,
			// maxPriorityFeePerGas: BigNumber.from(2),
			// maxFeePerGas: BigNumber.from(200000208912),
			gasPrice: null,
			// gasLimit: BigNumber.from(722000),
			to: ethers.utils.getAddress(contract),
			// value: BigNumber.from(0),
			data: input.data,
			accessList: [],
			// v: 1226,
			// r: '0xff8ff25480f5e1d1b38603b8fa1f10d64faf81707768dd9016fc4dd86d5474d2',
			// s: '0x6c2cfd5acd5b0b820e1c107efd5e7ce2c452b81742091f43f5c793a835c8644f',
			from: '0x14791697260E4c9A71f18484C9f997B308e59325',
			// hash: '0x456d37c868520b362bbf5baf1b19752818eba49cc92c1a512e2e80d1ccfbc18b',
		});
		expect(rawtx.maxPriorityFeePerGas?.toNumber()).to.equal(2);
		expect(rawtx.maxFeePerGas?.toNumber()).to.equal(200000208912);
		expect(rawtx.gasLimit?.toNumber()).to.equal(722000);
		expect(rawtx.value?.toNumber()).to.equal(0);


		// tx data to user input
		const input_storage_entry_limit = tx_gas_price.and(0xffff);
		const input_storage_limit = input_storage_entry_limit.mul(64);
		const input_block_period = (tx_gas_price.sub(input_storage_entry_limit).sub(tx_fee_per_gas).toNumber()) >> 16;
		const input_valid_until = input_block_period * 30;
		const input_gas_limit = tx_gas_limit.sub(storage_entry_deposit.div(tx_fee_per_gas).mul(input_storage_entry_limit));

		const tx = context.provider.api.tx.evm.ethCall(
			{ Call: value.to },
			value.data as any,
			value.value,
			input_gas_limit.toNumber(),
			input_storage_limit.toNumber(),
			value.accessList,
			input_valid_until
		);

		const sig = ethers.utils.joinSignature({ r: rawtx.r!, s: rawtx.s, v: rawtx.v })

		tx.addSignature(subAddr, { Eip1559: sig } as any, {
			blockHash: '0x', // ignored
			era: "0x00", // mortal
			genesisHash: '0x', // ignored
			method: "Bytes", // don't know that is this
			nonce: nonce,
			specVersion: 0, // ignored
			tip: tip,
			transactionVersion: 0, // ignored
		});

		expect(tx.toString()).to.equal(
			`{
				"signature": {
				  "signer": {
					"id": "5EMjsczQH4R2WZaB5Svau8HWZp1aAfMqjxfv3GeLWotYSkLc"
				  },
				  "signature": {
					"eip1559": "${sig}"
				  },
				  "era": {
					"immortalEra": "0x00"
				  },
				  "nonce": 1,
				  "tip": ${tip}
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
					"storage_limit": 1024,
					"access_list": [],
					"valid_until": 120
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
