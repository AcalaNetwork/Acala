import { expect, beforeAll, it } from "vitest";
import { describeWithAcala, nextBlock } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import EmptyContract from "../build/EmptyContract.json"

describeWithAcala("Acala RPC (EVM create fill block)", (context) => {
    let alice: BodhiSigner;

    beforeAll(async function () {
        [alice] = context.wallets;
    });

    it("evm create fill block", async function () {
        const bytecode = '0x' + EmptyContract.bytecode;
        const creates = Array(250).fill(context.provider.api.tx.evm.create(
            bytecode,
            0,
            2_000_000,
            100_000,
            []
        ));

        const beforeHeight = (await context.provider.api.query.system.number()).toNumber();
        let nonce = (await context.provider.api.query.system.account(alice.substrateAddress)).nonce.toNumber();

        for (const tx of creates) {
            await tx.signAndSend(alice.substrateAddress, { nonce: nonce++ });
        }

        await nextBlock(context);

        let currentBlockHash = await context.provider.api.rpc.chain.getBlockHash(beforeHeight + 1);

        const events = await context.provider.api.derive.tx.events(currentBlockHash);

        const evmCreateEvents = events.events.filter((item) => context.provider.api.events.evm.Created.is(item.event));

        expect(evmCreateEvents.length).to.equal(225);
    });
});
