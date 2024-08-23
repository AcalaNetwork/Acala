import { expect, beforeAll, it } from "vitest";
import ECRecoverTests from "../build/ECRecoverTests.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { BigNumber, Contract, ethers, Signer, Wallet } from "ethers";

describeWithAcala("Acala RPC (Precompile)", (context) => {
	let alice: Signer;
	let signer: Wallet;
	let contract: Contract;

	beforeAll(async () => {
		[alice] = context.wallets;
		contract = await deployContract(alice, ECRecoverTests);
		signer = new Wallet(
			"0x99B3C12287537E38C90A9219D4CB074A89A16E9CDB20BF85728EBD97C343E342"
		);
	});

	it('should perform ecrecover', async function () {
		const message = 'Lorem ipsum dolor sit amet, consectetur adipiscing elit. Tubulum fuisse, qua illum, cuius is condemnatus est rogatione, P. Eaedem res maneant alio modo.'
		const sig = (await signer.signMessage(message)).slice(2);

		const r = `${sig.slice(0, 64)}`
		const s = `${sig.slice(64, 128)}`
		const v = `${sig.slice(128, 130)}`
		const sigPart = `${Buffer.alloc(31).toString('hex')}${v}${r}${s}`;

		const hash = ethers.utils.keccak256("0x" + Buffer.from('\x19Ethereum Signed Message:\n' + message.length + message).toString('hex')).slice(2);

		const res = await contract.ecrecoverTest(`0x${hash.toString()}${sigPart}`);
		expect(res).to.deep.include({
			//hash: '0x14a18665b97477ba224a133a82798f2f895dfa13902a73be6199473aa13a8465',
			from: await alice.getAddress(),
			confirmations: 0,
			nonce: 1,
			// gasLimit: BigNumber.from("100200"),
			// gasPrice: BigNumber.from("1"),
			//data: "",
			// value: BigNumber.from(0),
			chainId: 595,
		});
		expect(res.gasLimit.toNumber()).toMatchInlineSnapshot(`200207`)
		expect(res.gasPrice.toNumber()).to.eq(1)
		expect(res.value.toNumber()).to.eq(0)

		expect(await context.provider.call({
			to: '0x0000000000000000000000000000000000000001',
			from: await alice.getAddress(),
			data: `0x${hash.toString()}${sigPart}`,
		})).to.equal("0x" + (await signer.getAddress()).toLowerCase().slice(2).padStart(64, '0'));
	});

	it('should perform identity directly', async () => {
		const message = '0x1234567890'
		const callResult = await context.provider.call({
			to: '0x0000000000000000000000000000000000000004',
			from: await alice.getAddress(),
			data: message,
		});
		expect(callResult).to.equal(message);
	});

	it('Precompile call should not panic', async function () {
		// currencies_len is MAX_UINT32
		const input = '0xcfea5c46000000000000000000000000100000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000000000000000ffffffff0000000000000000000000001000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000100000000000000000000000010000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000002503000101000202020202020202020202020202020202020202020202020202020202020202000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000901821a0600020004000000000000000000000000000000000000000000000000';

		await expect(async () => await context.provider.call({
			to: '0x000000000000000000000000000000000000040b',
			// Passes system contract filter
			from: '0x0000000000000000000100000000000000000001',
			data: input,
		})).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: invalid currencies size]`);
	});
});
