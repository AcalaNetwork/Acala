import { expect } from "chai";

import Block from "../build/Block.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { BigNumber } from "ethers";

describeWithAcala("Acala RPC (Gas)", (context) => {
	let alice: Signer;

	before("create the contract", async function () {
		this.timeout(15000);
		[alice] = await context.provider.getWallets();
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

		expect(data.gas.toNumber()).to.be.eq(273373);
		expect(data.storage.toNumber()).to.be.eq(10921);
		expect(data.weightFee.toNumber()).to.be.eq(5827787382433);
	});

	it("eth_estimateGas for contract call", async function () {
		const contract = await deployContract(alice as any, Block);
		const gas = await contract.estimateGas.multiply(3);
		expect(gas.toNumber()).to.be.eq(342409);
	});

	it("eth_estimateResources for contract call", async function () {
		const contract = await deployContract(alice as any, Block);

		const data = await context.provider.estimateResources(
			await contract.populateTransaction.multiply(3)
		);

		expect(data.gas.toNumber()).to.be.eq(22409);
		expect(data.storage.toNumber()).to.be.eq(0);
		expect(data.weightFee.toNumber()).to.be.eq(5827759352067);
	});
});
