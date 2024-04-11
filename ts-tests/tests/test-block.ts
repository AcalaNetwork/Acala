import { expect } from "chai";
import { deployContract } from "ethereum-waffle";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";
import Block from "../build/Block.json";
import { Contract } from "ethers";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (Block)", (context) => {
	step("should be at block 0 at genesis", async function () {
		expect(await context.provider.getBlockNumber()).to.equal(0);
	});

	step("should return genesis block by number", async function () {
		expect(await context.provider.getBlockNumber()).to.equal(0);
	});
});
