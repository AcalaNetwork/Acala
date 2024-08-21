import { expect, beforeAll, it } from "vitest";

import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { BigNumber, Contract } from "ethers";
import Block from "../build/Block.json"
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (bodhi.js)", (context) => {
	let alice: BodhiSigner;
	let contract: Contract;

	beforeAll(async () => {
		[alice] = context.wallets;
		contract = await deployContract(alice, Block);
	});

	it("should get client network", async function () {
		expect(await context.provider.getNetwork()).to.include({
			name: "mandala",
			chainId: 595
		});
	});

	it("should get block height", async function () {
		const height = await context.provider.getBlockNumber();
		expect(height.toString()).to.be.equal("6");
	});

	it("should get gas price", async function () {
		const gasPrice = await context.provider.getGasPrice();
		expect(gasPrice.toString()).toMatchInlineSnapshot(`"100000000206"`);
	});

	it("should get fee data ", async function () {
		const data = await context.provider.getFeeData();

		expect(data.gasPrice?.toNumber()).toMatchInlineSnapshot(`100000000206`);
	});

	it("should get transaction count", async function () {
		expect(await context.provider.getTransactionCount(await alice.getAddress())).to.be.equal(1);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), "latest")).to.be.equal(1);
		//expect(await context.provider.getTransactionCount(await alice.getAddress(), "pending")).to.be.equal(1);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), "earliest")).to.be.equal(0);
	});

	it("should get code", async function () {
		const code = await context.provider.getCode(contract.address);
		expect(code.length).to.be.equal(1950);
	});

	it("should storage at", async function () {
		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "latest"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
		//expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "pending"))
		//	.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "earliest"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
	});

	it("should call", async function () {
		expect(await context.provider.call(
			await contract.populateTransaction.multiply(3)
		)).to.equal("0x0000000000000000000000000000000000000000000000000000000000000015");

		expect(await context.provider.call(
			await contract.populateTransaction.multiply(3), "latest"
		)).to.equal("0x0000000000000000000000000000000000000000000000000000000000000015");

		//expect(await context.provider.call(
		//	await contract.populateTransaction.multiply(3), "pending"
		//)).to.equal("0x0000000000000000000000000000000000000000000000000000000000000015");

		// TODO: decide if we want to support for earliest
	});

	it("should estimateGas", async function () {
		const gas = await context.provider.estimateGas(
			await contract.populateTransaction.multiply(3)
		);

		expect(gas.toNumber()).toMatchInlineSnapshot(`200107`);
	});

	it("should estimateResources", async function () {
		const data = await context.provider.estimateResources(
			await contract.populateTransaction.multiply(3)
		);

		expect(data.usedGas.toNumber()).to.be.eq(22111);
		expect(data.safeStorage.toNumber()).toMatchInlineSnapshot(`70`);
		expect(data.gasLimit.toNumber()).to.closeTo(26445, 1000);
	});
});
