import { expect, beforeAll, it } from "vitest";
import TestCalls from "../build/TestCalls.json"
import { describeWithAcala, submitExtrinsic } from "./util";
import { deployContract } from "ethereum-waffle";
import { Contract } from "ethers";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (Precompile Filter Calls)", (context) => {
	let alice: BodhiSigner;
	let contract: Contract;

	const ecrecover = '0x0000000000000000000000000000000000000001';
	const identity = '0x0000000000000000000000000000000000000004';
	const ecrecoverPublic = '0x0000000000000000000000000000000000000080';

	const input = '0x18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549';

	const expect_addr = '0x000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b';
	const expect_pk = '0x3a514176466fa815ed481ffad09110a2d344f6c9b78c1d14afc351c3a51be33d8072e77939dc03ba44790779b7a1025baf3003f6732430e20cd9b76d953391b3';

	beforeAll(async function () {
		[alice] = context.wallets;
		contract = await deployContract(alice, TestCalls);
	});

	it('call non-standard precompile should not work with DELEGATECALL', async function () {
		expect(await contract.test_static_call(ecrecoverPublic, input)).to.be.eq(expect_pk);
		await contract.test_call(ecrecoverPublic, input, expect_pk);
		await expect(contract.test_delegate_call(ecrecoverPublic, input, expect_pk)).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: cannot be called with DELEGATECALL or CALLCODE]`);
	});

	it('call non-standard precompile should work with CALL and STATICCALL', async function () {
		expect(await contract.test_static_call(ecrecoverPublic, input)).to.be.eq(expect_pk);
		await contract.test_call(ecrecoverPublic, input, expect_pk);
	});

	it('call standard precompile should work with CALL, STATICCALL and DELEGATECALL', async function () {
		expect(await contract.test_static_call(ecrecover, input)).to.be.eq(expect_addr);
		await contract.test_call(ecrecover, input, expect_addr);
		await contract.test_delegate_call(ecrecover, input, expect_addr);
	});


	it('standard precompiles can be called directly', async function () {
		expect(await context.provider.call({
			to: ecrecover,
			from: await alice.getAddress(),
			data: input,
		}), expect_pk);
	});

	it('Acala precompiles cannot be called directly', async function () {
		await expect(context.provider.call({
			to: '0x0000000000000000000000000000000000000400',
			from: await alice.getAddress(),
			data: input,
		})).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: NoPermission]`);

		await expect(context.provider.call({
			to: '0x0000000000000000000000000000000000000400',
			from: '0x0000000000000000000111111111111111111111',
			data: input,
		})).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: Caller is not a system contract]`);

		// 41555344 -> AUSD
		expect(await context.provider.call({
			to: '0x0000000000000000000000000000000000000400',
			from: '0x0000000000000000000100000000000000000001',
			data: '0x95d89b410000000000000000000000000000000000000000000100000000000000000001',
		})).to.be.eq("0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000044155534400000000000000000000000000000000000000000000000000000000");
	});

	it('pause precompile', async function () {
		expect(await context.provider.call({
			to: identity,
			data: '0xff',
		})).to.be.eq('0xff');

		expect(await contract.test_static_call(identity, '0xff')).to.be.eq('0xff');
		await contract.test_call(identity, '0xff', '0xff');

		let nonce = (await context.provider.api.query.system.account(alice.substrateAddress)).nonce.toNumber();

		// pause precompile
		await submitExtrinsic(context.provider.api.tx.sudo.sudo(context.provider.api.tx.transactionPause.pauseEvmPrecompile(identity)), alice.substrateAddress, nonce);

		// calling precompile will error
		await expect(context.provider.call({
			to: identity,
			data: '0xff',
		})).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: precompile is paused]`);

		// contracts calling paused precompile will revert
		await expect(contract.test_static_call(identity, '0xff')).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: precompile is paused]`);
		await expect(contract.test_call(identity, '0xff', '0xff')).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: precompile is paused]`);

		// unpause precompile
		await submitExtrinsic(context.provider.api.tx.sudo.sudo(context.provider.api.tx.transactionPause.unpauseEvmPrecompile(identity)), alice.substrateAddress, nonce + 1);

		expect(await context.provider.call({
			to: identity,
			data: '0xff',
		})).to.be.eq('0xff');

		expect(await contract.test_static_call(identity, '0xff')).to.be.eq('0xff');
		await contract.test_call(identity, '0xff', '0xff');
	});
});
