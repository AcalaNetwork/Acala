import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";
import { ethers } from "ethers";

describeWithAcala("Acala RPC (Balance)", (context) => {
	let alice: Signer;

	before("init wallets", async function () {
		[ alice ] = await context.provider.getWallets();
	});

	step("genesis balance is setup correctly", async function () {
		expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999986219144000");
		expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString()).to.equal("8999999986219144000");

		expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString())
			.to.equal((await context.provider.api.query.system.account(await alice.getSubstrateAddress())).data.free.toString());
	});

	step("balance to be updated after transfer", async function () {
		this.timeout(15000);
		const to = await ethers.Wallet.createRandom().getAddress();

		await context.provider.api.tx.evm.call(to, "", "100000000000000", "210000", "1000").signAndSend(await alice.getSubstrateAddress());

		expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999986219144000");
		expect((await context.provider.getBalance(to)).toString()).to.equal("0");

		//let current_block_number = Number(await context.provider.api.query.system.number());
		//let block_hash = await context.provider.api.query.system.blockHash(current_block_number);
		//const data = await context.provider.api.derive.tx.events(block_hash);

		//console.log(data);
	});
});
