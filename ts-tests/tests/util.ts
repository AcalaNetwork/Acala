import { BodhiProvider, BodhiSigner, getTestUtils } from "@acala-network/bodhi";
import { Option } from '@polkadot/types/codec';
import { EvmAccountInfo } from '@acala-network/types/interfaces';
import { spawn, ChildProcess } from "child_process";
import chaiAsPromised from "chai-as-promised";
import chai from "chai";
import getPort from 'get-port';

export interface TestContext {
	provider: BodhiProvider;
	wallets: BodhiSigner[];
};

chai.use(chaiAsPromised);

export const DISPLAY_LOG = process.env.ACALA_LOG || false;
export const ACALA_LOG = process.env.ACALA_LOG || "info";
export const ACALA_BUILD = process.env.ACALA_BUILD || "debug";

export const BINARY_PATH = `../target/${ACALA_BUILD}/acala`;
export const SPAWNING_TIME = 120000;

export async function startAcalaNode(autoClaim = true): Promise<{ binary: ChildProcess; } & TestContext> {
	const P2P_PORT = await getPort({ port: getPort.makeRange(19931, 22000) });
	const RPC_PORT = await getPort({ port: getPort.makeRange(19931, 22000) });

	const cmd = BINARY_PATH;
	const args = [
		`--dev`,
		`-lruntime=debug`,
		`-levm=debug`,
		`--instant-sealing`,
		`--no-telemetry`,
		`--no-prometheus`,
		`--port=${P2P_PORT}`,
		`--rpc-port=${RPC_PORT}`,
		`--rpc-external`,
		`--rpc-cors=all`,
		`--rpc-methods=unsafe`,
		`--pruning=archive`,
		`--keep-blocks=archive`,
		`--tmp`,
	];
	const binary = spawn(cmd, args);

	binary.on("error", (err) => {
		if ((err as any).errno == "ENOENT") {
			console.error(
				`\x1b[31mMissing Acala binary (${BINARY_PATH}).\nPlease compile the Acala project:\nmake test-ts\x1b[0m`
			);
		} else {
			console.error(err);
		}
		process.exit(1);
	});

	const binaryLogs = [] as any;
	const { provider, wallets } = await new Promise<TestContext>((resolve, reject) => {
		const timer = setTimeout(() => {
			console.error(`\x1b[31m Failed to start Acala Node.\x1b[0m`);
			console.error(`Command: ${cmd} ${args.join(" ")}`);
			console.error(`Logs:`);
			console.error(binaryLogs.map((chunk: any) => chunk.toString()).join("\n"));
			process.exit(1);
		}, SPAWNING_TIME - 2000);

		const onData = async (chunk: any) => {
			if (DISPLAY_LOG) {
				console.log(chunk.toString());
			}
			binaryLogs.push(chunk);
			if (chunk.toString().match(/best: #0/)) {
				try {
					const { provider, wallets } = await getTestUtils(`ws://127.0.0.1:${RPC_PORT}`, autoClaim);

					clearTimeout(timer);
					if (!DISPLAY_LOG) {
						binary.stderr.off("data", onData);
						binary.stdout.off("data", onData);
					}
					resolve({ provider, wallets });
				} catch(e) {
					binary.kill();
					reject(e);
				}
			}
		};
		binary.stderr.on("data", onData);
		binary.stdout.on("data", onData);
	});

	return { provider, wallets, binary };
}

export function describeWithAcala(title: string, cb: (context: TestContext) => void) {
	let context = {} as TestContext;

	describe(title, () => {
		let binary: ChildProcess;
		// Making sure the Acala node has started
		before("Starting Acala Test Node", async function () {
			console.log('starting acala node ...')
			this.timeout(SPAWNING_TIME);

			const autoClaim =
				title !== 'Acala RPC (Claim Account Eip712)' &&
				title !== 'Acala RPC (Block)';
			const init = await startAcalaNode(autoClaim);

			context.provider = init.provider,
			context.wallets = init.wallets,
			binary = init.binary;

			console.log('acala node started!')
		});

		after(async function () {
			//console.log(`\x1b[31m Killing RPC\x1b[0m`);
			context.provider.api.disconnect()
			binary.kill();
		});

		cb(context);
	});
}

export async function nextBlock(context: TestContext) {
	return new Promise(async (resolve) => {
		let [alice] = context.wallets;
		let block_number = await context.provider.api.query.system.number();
		context.provider.api.tx.system.remark(block_number.toString(16)).signAndSend(alice.substrateAddress, (result) => {
			if (result.status.isFinalized || result.status.isInBlock) {
				resolve(undefined);
			}
		});
	});
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
