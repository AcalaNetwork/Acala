import { expect, beforeAll, it } from "vitest";
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import ExplicitRevertReason from "../build/ExplicitRevertReason.json"
import { BodhiSigner } from "@acala-network/bodhi";
import { Contract } from "ethers";

describeWithAcala("Acala RPC (Revert Reason)", (context) => {
	let alice: BodhiSigner;
	let contract: Contract;

	beforeAll(async function () {
		[alice] = context.wallets;
		contract = await deployContract(alice, ExplicitRevertReason);
	});

	it("should fail with revert reason", async function () {
		await expect(contract.max10(30)).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution reverted: Value must not be greater than 10.]`);
	});
});
