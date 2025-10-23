import { expect, beforeAll, it } from "vitest";
import Block from "../build/Block.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (Gas)", (context) => {
	let alice: BodhiSigner;

	beforeAll(async function () {
		[alice] = context.wallets;
	});

	it("eth_estimateGas for contract creation", async function () {
		const gas = await context.provider.estimateGas({
			from: alice.getAddress(),
			data: "0x" + Block.bytecode,
		});
		expect(gas.toNumber()).to.closeTo(12601114, 1000);
	});

	it("eth_estimateResources for contract creation", async function () {
		const data = await context.provider.estimateResources({
			from: await alice.getAddress(),
			data: "0x" + Block.bytecode,
		});

		expect(data.usedGas.toNumber()).toMatchInlineSnapshot(`263596`)
		expect(data.gasLimit.toNumber()).closeTo(316315, 1000);
		expect(data.safeStorage.toNumber()).toMatchInlineSnapshot(`12141`)
	});

	it("eth_estimateGas for contract call", async function () {
		const contract = await deployContract(alice, Block);
		const gas = await contract.estimateGas.multiply(3);
		expect(gas.toNumber()).toMatchInlineSnapshot(`200107`)
	});

	it("eth_estimateResources for contract call", async function () {
		const contract = await deployContract(alice, Block);

		const data = await context.provider.estimateResources(
			await contract.populateTransaction.multiply(3)
		);

		expect(data.usedGas.toNumber()).toMatchInlineSnapshot(`22111`)
		expect(data.gasLimit.toNumber()).to.closeTo(26445, 1000);
		expect(data.safeStorage.toNumber()).toMatchInlineSnapshot(`70`)
	});
});
