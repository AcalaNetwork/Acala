import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala, transfer } from "./util";

describeWithAcala("Acala RPC (Balance)", (context) => {
	let alice: Signer;
	let alice_stash: Signer;

	before("init wallets", async function () {
		[alice, alice_stash] = await context.provider.getWallets();
	});

	step("genesis balance is setup correctly", async function () {
		expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999985535771315000000");
		expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString()).to.equal("8999999985535771315000000");

		expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString())
			.to.equal((await context.provider.api.query.system.account(await alice.getSubstrateAddress())).data.free.toString() + "000000");
	});

	step("balance to be updated after transfer", async function () {
		this.timeout(15000);

		expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999985535771315000000");
		expect((await context.provider.getBalance(alice_stash.getAddress())).toString()).to.equal("10100000985535778201000000");

		await transfer(context, await alice.getSubstrateAddress(), await alice_stash.getSubstrateAddress(), 1000);
		expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999968467638103000000");
		expect((await context.provider.getBalance(alice_stash.getAddress())).toString()).to.equal("10100000985535779201000000");
		expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString()).to.equal("8999999968467638103000000");
		expect((await context.provider.getBalance(alice_stash.getAddress(), "earliest")).toString()).to.equal("0");
	});
});
