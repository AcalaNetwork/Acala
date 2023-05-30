import { expect } from "chai";

import Block from "../build/Block.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (Gas)", (context) => {
	let alice: BodhiSigner;

	before("create the contract", async function () {
		this.timeout(15000);
		[alice] = context.wallets;
	});

	it("eth_estimateGas for contract creation", async function () {
		const gas = await context.provider.estimateGas({
			from: alice.getAddress(),
			data: "0x" + Block.bytecode,
		});
		expect(gas.toNumber()).to.be.eq(593373);
	});

	it("eth_estimateResources for contract creation", async function () {
		const data = await context.provider.estimateResources({
			from: await alice.getAddress(),
			data: "0x" + Block.bytecode,
		});

		expect(data.usedGas.toNumber()).to.be.eq(251726);
		expect(data.gasLimit.toNumber()).to.be.eq(273373);
		expect(data.usedStorage.toNumber()).to.be.eq(10921);
	});

	it("eth_estimateGas for contract call", async function () {
		const contract = await deployContract(alice, Block);
		const gas = await contract.estimateGas.multiply(3);
		expect(gas.toNumber()).to.be.eq(342409);
	});

	it("eth_estimateResources for contract call", async function () {
		const contract = await deployContract(alice, Block);

		const data = await context.provider.estimateResources(
			await contract.populateTransaction.multiply(3)
		);

		expect(data.usedGas.toNumber()).to.be.eq(22038);
		expect(data.gasLimit.toNumber()).to.be.eq(22409);
		expect(data.usedStorage.toNumber()).to.be.eq(0);
	});
});