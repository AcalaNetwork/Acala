import { expect } from "chai";
import { step } from "mocha-steps";

import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import Block from "../build/Block.json"

describeWithAcala("Acala RPC (bodhi.js)", (context) => {
	step("should get client network", async function () {
		const network = await context.provider.getNetwork();
		expect(network.name).to.be.equal("mandala");
		expect(network.chainId).to.be.equal(595);
	});

	step("should get gas price", async function () {
		const gasPrice = await context.provider.getGasPrice();
		expect(gasPrice.toString()).to.be.equal("1");
	});

	step("should get block height", async function () {
		const height = await context.provider.getBlockNumber();
		expect(height.toString()).to.be.equal("0");
	});

	step("should get code", async function () {
		const [ alice ] = await context.provider.getWallets();
		const contract = await deployContract(alice as any, Block);

		const code = await context.provider.getCode(contract.address);
		expect(code.length).to.be.equal(1334);
	});
});
