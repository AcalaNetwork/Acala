import { expect, it } from "vitest";
import { describeWithAcala, transfer } from "./util";
import { deployContract } from "ethereum-waffle";
import { ethers } from "ethers";
import Erc20DemoContract from "../build/Erc20DemoContract.json"

describeWithAcala("Acala RPC (Nonce)", (context) => {
	it("get nonce", async function () {
		const [alice, alice_stash] = context.wallets;

		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'earliest')).to.eq(0);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(0);

		await transfer(context, alice.substrateAddress, alice_stash.substrateAddress, 1000);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(0);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'pending')).to.eq(0);

		const contract = await deployContract(alice, Erc20DemoContract, [1000000000]);
		const to = await ethers.Wallet.createRandom().getAddress();

		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(1);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'pending')).to.eq(1);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'earliest')).to.eq(0);

		await contract.transfer(to, 1000);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(2);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'pending')).to.eq(2);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'earliest')).to.eq(0);

		// TODO: tx pool pending
	});
});
