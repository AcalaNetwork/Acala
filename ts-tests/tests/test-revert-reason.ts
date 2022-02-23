import { expect } from "chai";

import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import ExplicitRevertReason from "../build/ExplicitRevertReason.json"

describeWithAcala("Acala RPC (Revert Reason)", (context) => {
	let alice: Signer;
	let contract: Contract;

	before("create the contract", async function () {
		this.timeout(15000);
		[alice] = await context.provider.getWallets();
		contract = await deployContract(alice as any, ExplicitRevertReason);
	});

	it("should fail with revert reason", async function () {
		await expect(contract.max10(30)).to.be.rejectedWith({ message: '-32603: VM Exception while processing transaction: execution revert: Value must not be greater than 10.'});
	});
});
