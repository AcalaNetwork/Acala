import { expect } from "chai";

import Factory from "../build/Factory.json"
import { describeWithAcala } from "./util";
import { deployContract } from "ethereum-waffle";
import { Option } from "@polkadot/types/codec";
import { u32 } from "@polkadot/types";
import { EvmAccountInfo, CodeInfo } from "@acala-network/types/interfaces";

describeWithAcala("Acala RPC (GasLimit)", (context) => {
	let alice: Signer;

    before(async () => {
        [alice] = await context.provider.getWallets();
    });

    it("block gas limit", async () => {
        const contract = await deployContract(alice as any, Factory);
        // limited by used_storage
        const result = await contract.createContractLoop(360);
        expect(result.gasLimit.toNumber()).to.be.eq(30725309);

        const result2 = await contract.incrementLoop(9_500);
        expect(result2.gasLimit.toNumber()).to.be.eq(32803507);

        const storages = await context.provider.api.query.evm.accountStorages.entries(contract.address);
        // 360 array items
        // 1 array length
        // 1 increment value
        expect(storages.length).to.be.eq(362);

        const info = await context.provider.api.query.evm.accounts(contract.address) as Option<EvmAccountInfo>;
        const codeInfo = await context.provider.api.query.evm.codeInfos(info.unwrap().contractInfo.unwrap().codeHash) as Option<CodeInfo>;
        const extra_bytes = Number(context.provider.api.consts.evm.newContractExtraBytes.toHex());

        const contract_total_storage = await context.provider.api.query.evm.contractStorageSizes(contract.address) as u32;

        expect(contract_total_storage.toNumber()).to.be.eq(storages.length * 64 + codeInfo.unwrap().codeSize.toNumber() + extra_bytes);
    });
});