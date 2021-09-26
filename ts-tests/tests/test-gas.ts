import { expect } from "chai";

import Block from "../build/Block.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";

describeWithAcala("Acala RPC (Gas)", (context) => {
	let alice: Signer;

	before("create the contract", async function () {
		this.timeout(15000);
		[ alice ] = await context.provider.getWallets();
	});

	it("eth_estimateGas for contract creation", async function () {
		expect(
			(await context.provider.estimateGas({
				from: alice.getAddress(),
				data: "0x" + Block.bytecode,
			})).toString()
		).to.equal("207311");
	});

	it("eth_estimateResources for contract creation", async function () {
		const resource = await context.provider.estimateResources({
			from: await alice.getAddress(),
			data: "0x" + Block.bytecode,
		});
		expect(resource.gas.toString()).to.equal("196645");
		expect(resource.storage.toString()).to.equal("10666");
		expect(resource.weightFee.toString()).to.equal("0");
	});

	it("eth_estimateGas for contract call", async function () {
		const contract = await deployContract(alice as any, Block);

		expect((await contract.estimateGas.multiply(3)).toString()).to.equal("22016");
	});

	it("eth_estimateResources for contract call", async function () {
		const contract = await deployContract(alice as any, Block);

		const resource = await context.provider.estimateResources(
			contract.populateTransaction.multiply(3)
		);
		expect(resource.gas.toString()).to.equal("22016");
		expect(resource.storage.toString()).to.equal("0");
		expect(resource.weightFee.toString()).to.equal("0");
	});
});
