import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";
import { ethers } from "ethers";

describeWithAcala("Acala RPC (Block)", (context) => {
	step("should be at block 0 at genesis", async function () {
		expect(await context.provider.getBlockNumber()).to.equal(0);
	});

	it("should return genesis block by number", async function () {
		expect(await context.provider.getBlockNumber()).to.equal(0);

		// Unimplemented
		// const block = await context.provider.getBlock(0);
		// expect(block).to.include({
		// 	author: "0x0000000000000000000000000000000000000000",
		// 	difficulty: "0",
		// 	extraData: "0x",
		// 	gasLimit: 4294967295,
		// 	gasUsed: 0,
		// 	logsBloom:
		// 		"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
		// 	miner: "0x0000000000000000000000000000000000000000",
		// 	number: 0,
		// 	receiptsRoot: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
		// 	size: 505,
		// 	timestamp: 0,
		// 	totalDifficulty: "0",
		// });

		// expect((block as any).sealFields).to.eql([
		// 	"0x0000000000000000000000000000000000000000000000000000000000000000",
		// 	"0x0000000000000000",
		// ]);
		// expect(block.hash).to.be.a("string").lengthOf(66);
		// expect(block.parentHash).to.be.a("string").lengthOf(66);
		// expect(block.timestamp).to.be.a("number");
		// previousBlock = block;
	});

});
