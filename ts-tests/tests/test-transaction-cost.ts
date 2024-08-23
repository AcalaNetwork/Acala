import { expect, it } from "vitest";

import { ethers } from "ethers";
import { deployContract } from "ethereum-waffle";
import { describeWithAcala } from "./util";
import Erc20DemoContract from "../build/Erc20DemoContract.json"

describeWithAcala("Acala RPC (Transaction cost)", (context) => {

	it("should take transaction cost into account and not submit it to the pool", async function () {
		const [alice] = context.wallets;
		const contract = await deployContract(alice, Erc20DemoContract, [1000000000]);
		const to = await ethers.Wallet.createRandom().getAddress();

		await expect(contract.transfer(to, 1000, { gasLimit: 0 })).rejects.toThrowErrorMatchingInlineSnapshot(`[Error: execution error: outOfGas]`);
	});
});
