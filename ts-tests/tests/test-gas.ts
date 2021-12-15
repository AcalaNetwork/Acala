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
		expect(
			await context.provider.estimateGas({
				from: alice.getAddress(),
				data: "0x" + Block.bytecode,
			})
		).to.deep.equal(BigNumber.from("273373"));
	});

	it("eth_estimateResources for contract creation", async function () {
		expect(await context.provider.estimateResources({
			from: await alice.getAddress(),
			data: "0x" + Block.bytecode,
		})).to.deep.include({
			gas: BigNumber.from("273373"),
			storage: BigNumber.from("10921"),
			weightFee: BigNumber.from("3999960176247")
		});
	});

	it("eth_estimateGas for contract call", async function () {
		const contract = await deployContract(alice as any, Block);

		expect(await contract.estimateGas.multiply(3)).to.deep.equal(BigNumber.from("22409"));
	});

	it("eth_estimateResources for contract call", async function () {
		const contract = await deployContract(alice as any, Block);

		expect(await context.provider.estimateResources(
			await contract.populateTransaction.multiply(3)
		)).to.deep.include({
			gas: BigNumber.from("22409"),
			storage: BigNumber.from("0"),
			weightFee: BigNumber.from("3999940942452")
		});
	});
});
