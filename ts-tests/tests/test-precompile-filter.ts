import { expect } from "chai";

import TestCalls from "../build/TestCalls.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { Contract, Signer } from "ethers";

describeWithAcala("Acala RPC (Precompile Filter Calls)", (context) => {
	let alice: Signer;
	let contract: Contract;

	const ecrecover = '0x0000000000000000000000000000000000000001';
	const ecrecoverPublic = '0x0000000000000000000000000000000000000080';

	const input = '0x18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549';

	const expect_addr = '0x000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b';
	const expect_pk = '0x3a514176466fa815ed481ffad09110a2d344f6c9b78c1d14afc351c3a51be33d8072e77939dc03ba44790779b7a1025baf3003f6732430e20cd9b76d953391b3';

	before("create the contract", async function () {
		this.timeout(15000);
		[alice] = await context.provider.getWallets();
		contract = await deployContract(alice as any, TestCalls);
	});

	it('call non-standard precompile should not work with DELEGATECALL', async function () {
		expect(await contract.test_static_call(ecrecoverPublic, input)).to.be.eq(expect_pk);
		await contract.test_call(ecrecoverPublic, input, expect_pk);
		await expect(contract.test_delegate_call(ecrecoverPublic, input, expect_pk)).to.be.rejectedWith("cannot be called with DELEGATECALL or CALLCODE");
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
});
