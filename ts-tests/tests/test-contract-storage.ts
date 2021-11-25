import { expect } from "chai";

import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import Storage from "../build/Storage.json"

describeWithAcala("Acala RPC (Contract)", (context) => {
	it("eth_getStorageAt", async function () {
		this.timeout(15000);

		const [alice] = await context.provider.getWallets();
		const contract = await deployContract(alice as any, Storage);

		expect(await contract.getStorage("0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");

		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");


		await contract.setStorage("0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");

		expect(await contract.getStorage("0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"))
			.to.equal("0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");

		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"))
			.to.equal("0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");

		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "earliest"))
			.to.equal("0x0000000000000000000000000000000000000000000000000000000000000000");
		expect(await context.provider.getStorageAt(contract.address, "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc", "latest"))
			.to.equal("0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
	});
});
