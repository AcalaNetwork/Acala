import { expect, beforeAll, it } from "vitest";
import Factory from "../build/Factory.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { Option } from "@polkadot/types/codec";
import { u32 } from "@polkadot/types";
import { CodeInfo } from "@acala-network/types/interfaces";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (GasLimit)", (context) => {
	let alice: BodhiSigner;

    beforeAll(async () => {
        [alice] = context.wallets;
    });

    it("block gas limit", async () => {
        const contract = await deployContract(alice, Factory);
        // limited by used_storage
        const result = await contract.createContractLoop(350);
        expect(result.gasLimit.toNumber()).toMatchInlineSnapshot(`3928498122`);

        const result2 = await contract.incrementLoop(8130);
        expect(result2.gasLimit.toNumber()).toMatchInlineSnapshot(`32506508`);

        const storages = await context.provider.api.query.evm.accountStorages.entries(contract.address);
        // 350 array items
        // 1 array length
        // 1 increment value
        expect(storages.length).to.be.eq(352);

        const info = await context.provider.api.query.evm.accounts(contract.address);
        const codeInfo = await context.provider.api.query.evm.codeInfos(info.unwrap().contractInfo.unwrap().codeHash) as Option<CodeInfo>;
        const extra_bytes = Number(context.provider.api.consts.evm.newContractExtraBytes.toHex());

        const contract_total_storage = await context.provider.api.query.evm.contractStorageSizes(contract.address) as u32;

        expect(contract_total_storage.toNumber()).to.be.eq(storages.length * 64 + codeInfo.unwrap().codeSize.toNumber() + extra_bytes);
        expect(contract_total_storage).toMatchInlineSnapshot(`33203`);
    });
});
