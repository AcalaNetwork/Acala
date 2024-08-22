import { Blockchain, BuildBlockMode, setupWithServer } from "@acala-network/chopsticks";
import { BodhiProvider, BodhiSigner, getTestUtils } from "@acala-network/bodhi";
import { Option } from '@polkadot/types/codec';
import { EvmAccountInfo } from '@acala-network/types/interfaces';
import { AddressOrPair, SubmittableExtrinsic } from "@polkadot/api/types";
import { afterAll, beforeAll, describe } from "vitest";
import "chai";

export interface TestContext {
	provider: BodhiProvider;
	wallets: BodhiSigner[];
	chain: Blockchain
	close: () => Promise<void>;
};

export async function startAcalaNode(sealing = true, autoClaim = true): Promise<TestContext> {
	const server = await setupWithServer({
		port: 0,
		'chain-spec': __dirname + '/../../chainspecs/dev.json',
		'build-block-mode': sealing ? BuildBlockMode.Instant : BuildBlockMode.Batch,
		'runtime-log-level': 0,
	});

	const { provider, wallets } = await getTestUtils(`ws://127.0.0.1:${server.listenPort}`, autoClaim);

	if (!sealing) {
		server.chain.txPool.mode = BuildBlockMode.Manual;
	}

	return { provider, wallets, chain: server.chain, close: server.close };
}

export function describeWithAcala(title: string, cb: (context: TestContext) => void) {
	let context = {} as TestContext;

	describe.sequential(title, () => {
		// Making sure the Acala node has started
		beforeAll(async function () {
			console.log('starting acala node ...')

			const sealing =
				title !== 'Acala RPC (EVM create fill block)' &&
				title !== 'Acala RPC (EVM call fill block)';

			const autoClaim =
				title !== 'Acala RPC (Claim Account Eip712)' &&
				title !== 'Acala RPC (Block)';
			const init = await startAcalaNode(sealing, autoClaim);
			Object.assign(context, init);

			console.log('acala node started!')
		});

		afterAll(async function () {
			// console.log(`\x1b[31m Killing RPC\x1b[0m`);
			await context.provider.api.disconnect()
			await context.close();
		});

		cb(context);
	});
}

export async function nextBlock(context: TestContext) {
	await context.chain.newBlock();
}

export async function transfer(context: TestContext, from: string, to: string, amount: number) {
	return new Promise(async (resolve) => {
		context.provider.api.tx.balances.transferAllowDeath(to, amount).signAndSend(from, (result) => {
			if (result.status.isFinalized || result.status.isInBlock) {
				resolve(undefined);
			}
		});
	});
}

export async function getEvmNonce(provider: BodhiProvider, address: string): Promise<number> {
	const evm_account = await provider.api.query.evm.accounts<Option<EvmAccountInfo>>(address);
	const nonce = evm_account.isEmpty ? 0 : evm_account.unwrap().nonce.toNumber();
	return nonce;
}

export async function submitExtrinsic(extrinsic: SubmittableExtrinsic<'promise'>, sender: AddressOrPair, nonce?: number) {
	return new Promise(async (resolve, reject) => {
		extrinsic.signAndSend(sender, { nonce }, (result) => {
			if (result.status.isFinalized || result.status.isInBlock) {
				resolve(undefined);
			}
		}).catch(reject);
	});
}
