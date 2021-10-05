import { expect } from "chai";
import { step } from "mocha-steps";

import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { ethers } from "ethers";
import Erc20DemoContract from "../build/Erc20DemoContract.json"

describeWithAcala("Acala RPC (Nonce)", (context) => {
	step("get nonce", async function () {
		this.timeout(10_000);
		const [ alice, alice_stash ] = await context.provider.getWallets();

		async function transfer() {
			return new Promise(async (resolve) => {
				let [ alice ] = await context.provider.getWallets();
				context.provider.api.tx.balances.transfer(await alice_stash.getSubstrateAddress(), 1000).signAndSend(await alice.getSubstrateAddress(), (result) => {
					if (result.status.isInBlock) {
						resolve(undefined);
					}
				});
			});
		}

		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'earliest')).to.eq(0);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(0);

		await transfer();
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(0);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'pending')).to.eq(0);

		const contract = await deployContract(alice as any, Erc20DemoContract, [1000000000]);
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
