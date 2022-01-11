import { expect } from "chai";
import { step } from "mocha-steps";

import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { BigNumber, Contract } from "ethers";
import Block from "../build/Block.json"

describeWithAcala("Acala RPC (bodhi.js)", (context) => {
	let alice: Signer;
	let contract: Contract;

	before(async () => {
		[alice] = await context.provider.getWallets();
		contract = await deployContract(alice as any, Block);
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
		expect(gasPrice.toString()).to.be.equal("1");
	});

	step("should get fee data ", async function () {
		expect(await context.provider.getFeeData()).to.deep.include({
			maxFeePerGas: BigNumber.from("1"),
			maxPriorityFeePerGas: BigNumber.from("1"),
			gasPrice: BigNumber.from("1"),
		});
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
		expect(await context.provider.estimateGas(
			await contract.populateTransaction.multiply(3)
		)).to.deep.equal(BigNumber.from("22409"));
	});

	step("should estimateResources", async function () {
		expect(await context.provider.estimateResources(
			await contract.populateTransaction.multiply(3)
		)).to.deep.include({
			gas: BigNumber.from("22409"),
			storage: BigNumber.from(0),
			weightFee: BigNumber.from("3999950906427"),
		});
	});
});
