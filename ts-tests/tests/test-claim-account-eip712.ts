import { expect } from "chai";

import { describeWithAcala } from "./util";
import { Signer, AccountSigningKey } from "@acala-network/bodhi";
import { Wallet } from "@ethersproject/wallet";
import { Keyring } from "@polkadot/keyring";
import { createTestKeyring } from "@polkadot/keyring/testing";

describeWithAcala("Acala RPC (Claim Account Eip712)", (context) => {
	let alice: Signer;
	let signer: Wallet;

	before("init", async function () {
		this.timeout(15000);

		// need to manually get key as the getWallets method claims accounts in evm
		const test_keyring = createTestKeyring();
		const alice_keyring = test_keyring.pairs[0];

		const signingKey = new AccountSigningKey(context.provider.api.registry);
		signingKey.addKeyringPair([alice_keyring]);

		await context.provider.api.isReady;

		alice = new Signer(context.provider, alice_keyring.address, signingKey);

		signer = new Wallet("0x0123456789012345678901234567890123456789012345678901234567890123");
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
			Transaction: [{ name: "substrateAddress", type: "bytes" }],
		};

		const keyring = new Keyring({ type: "sr25519", ss58Format: +context.provider.api.consts.system.ss58Prefix });
		const alice_addr = await alice.getSubstrateAddress();
		const public_key = keyring.decodeAddress(alice_addr);

		// The data to sign
		const value = {
			substrateAddress: public_key,
		};

		const signature = await signer._signTypedData(domain, types, value);
		const tx = context.provider.api.tx.evmAccounts.claimAccount(await signer.getAddress(), signature);

		await new Promise(async (resolve) => {
			tx.signAndSend(await alice.getSubstrateAddress(), (result) => {
				if (result.status.isFinalized || result.status.isInBlock) {
					resolve(undefined);
				}
			});
		});

		let current_block_number = (await context.provider.api.query.system.number()).toNumber();
		let block_hash = await context.provider.api.rpc.chain.getBlockHash(current_block_number);
		const result = await context.provider.api.derive.tx.events(block_hash);

		let event = result.events.filter((item) => context.provider.api.events.evmAccounts.ClaimAccount.is(item.event));
		expect(event.length).to.equal(1);
	});
});
