import { expect } from "chai";
import { step } from "mocha-steps";

import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { BigNumber, Contract } from "ethers";
import Block from "../build/Block.json"
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (bodhi.js)", (context) => {
	let alice: BodhiSigner;
	let contract: Contract;

	before(async () => {
		[alice] = context.wallets;
		contract = await deployContract(alice, Block);
	});

	step("should get client network", async function () {
		expect(await context.provider.getNetwork()).to.include({
			name: "mandala",
			chainId: 595
		});
	});

	step("should get block height", async function () {
		const height = await context.provider.getBlockNumber();
		expect(height.toString()).to.be.equal("5");
	});

	step("should get gas price", async function () {
		const gasPrice = await context.provider.getGasPrice();
		expect(gasPrice.toString()).to.be.equal("100000000205");
	});

	step("should get fee data ", async function () {
		const data = await context.provider.getFeeData();

		expect(data.gasPrice?.toNumber()).to.be.eq(100000000205);
	});

	step("should get transaction count", async function () {
		expect(await context.provider.getTransactionCount(await alice.getAddress())).to.be.equal(1);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), "latest")).to.be.equal(1);
		//expect(await context.provider.getTransactionCount(await alice.getAddress(), "pending")).to.be.equal(1);
		expect(await context.provider.getTransactionCount(await alice.getAddress(), "earliest")).to.be.equal(0);
	});

	step("should get code", async function () {
		const code = await context.provider.getCode(contract.address);
		expect(code.length).to.be.equal(1844);
	});

	step("should storage at", async function () {
		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "latest"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
		//expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "pending"))
		//	.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "earliest"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
	});

	step("should call", async function () {
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

	step("should estimateGas", async function () {
		const gas = await context.provider.estimateGas(
			await contract.populateTransaction.multiply(3)
		);

		expect(gas.toNumber()).to.be.eq(100100);
	});

	step("should estimateResources", async function () {
		const data = await context.provider.estimateResources(
			await contract.populateTransaction.multiply(3)
		);

		expect(data.usedGas.toNumber()).to.be.eq(22038);
		expect(data.usedStorage.toNumber()).to.be.eq(0);
		expect(data.gasLimit.toNumber()).to.be.eq(22409);
	});
});