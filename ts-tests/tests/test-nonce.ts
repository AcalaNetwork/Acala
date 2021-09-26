import { expect } from "chai";
import { step } from "mocha-steps";

import { describeWithAcala } from "./util";

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
		// claim evm address
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(1);

		await transfer();
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(2);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'pending')).to.eq(2);

		// TODO: tx pool pending
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'latest')).to.eq(2);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'pending')).to.eq(2);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), 'earliest')).to.eq(0);
	});
});
