import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";

describeWithAcala("Acala RPC (Block)", (context) => {
	step("should be at block 0 at genesis", async function () {
		expect(await context.provider.getBlockNumber()).to.equal(4);		// test utils created 4 wallets and claimed their evm addresses
	});

	it("should return genesis block by number", async function () {
		expect(await context.provider.getBlockNumber()).to.equal(4);
	});
});
