import { expect } from "chai";

import { describeWithAcala, nextBlock } from "./util";
import { Signer, TestAccountSigningKey, TestProvider } from "@acala-network/bodhi";
import { Wallet } from "@ethersproject/wallet";
import { createTestKeyring } from "@polkadot/keyring/testing";
import { stringToHex } from "@polkadot/util";

describeWithAcala("Acala RPC (Sign Claim Account eip712)", (context) => {
	let alice: Signer;
	let signer: Wallet;

	before("init", async function () {
		this.timeout(15000);

		// need to manually get key as the getWallets method claims accounts in evm
		const test_keyring = createTestKeyring();
		const alice_keyring = test_keyring.pairs[0];

		const signingKey = new TestAccountSigningKey(context.provider.api.registry);
		signingKey.addKeyringPair([alice_keyring]);

		await context.provider.api.isReady;

		alice = new Signer(context.provider, alice_keyring.address, signingKey);

		signer = new Wallet(
			"0x0123456789012345678901234567890123456789012345678901234567890123"
		);
	});

	it("claim evm account", async function () {
		this.timeout(150000);

		const domain = {
			name: "Acala EVM claim",
			version: "1",
			chainId: +context.provider.api.consts.evmAccounts.chainId.toString(),
			salt: (await context.provider.api.rpc.chain.getBlockHash(0)).toHex(),
		};

		const types = {
			Transaction: [
				{ name: "address", type: "string" },
			],
		};

		// The data to sign
		const value = {
			address: await alice.getSubstrateAddress(),
		};

		const signature = await signer._signTypedData(domain, types, value);

		console.log("")


        const tx = context.provider.api.tx.evmAccounts.claimAccount(await signer.getAddress(), signature);

        await new Promise(async (resolve) => {
			tx.signAndSend(await alice.getSubstrateAddress(), (result) => {
				if (result.status.isFinalized || result.status.isInBlock) {
					console.log(result.dispatchError?.toString());
					resolve(undefined);
				}
			});
        });

		let current_block_number = (await context.provider.api.query.system.number()).toNumber();
		let block_hash = await context.provider.api.rpc.chain.getBlockHash(current_block_number);
		const result = await context.provider.api.derive.tx.events(block_hash);
		// console.log("current_block_number: ", current_block_number, " event: ", result.events.toString());

		let event = result.events.filter(item => context.provider.api.events.evmAccounts.ClaimAccount.is(item.event));
		expect(event.length).to.equal(1);
	});
});
