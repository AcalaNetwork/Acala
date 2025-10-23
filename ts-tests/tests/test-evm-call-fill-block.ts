import { expect, beforeAll, it } from "vitest";
import { describeWithAcala, nextBlock } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (EVM call fill block)", (context) => {
    let alice: BodhiSigner;

    beforeAll(async function () {
        [alice] = context.wallets;
    });

    it("evm call fill block", async function () {
        const input = "0xa9059cbb0000000000000000000000001000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000174876e800";

        // transfer 100000000000 ACA
        const transfers = Array(300).fill(context.provider.api.tx.evm.call(
            "0x0000000000000000000100000000000000000000",
            input,
            0,
            100_000,
            100_000,
            []
        ));

        const beforeHeight = (await context.provider.api.query.system.number()).toNumber();
        let nonce = (await context.provider.api.query.system.account(alice.substrateAddress)).nonce.toNumber();

        for (const tx of transfers) {
            await tx.signAndSend(alice.substrateAddress, { nonce: nonce++ });
        }

        await nextBlock(context);

        let currentBlockHash = await context.provider.api.rpc.chain.getBlockHash(beforeHeight + 1);

        const events = await context.provider.api.derive.tx.events(currentBlockHash);

        const evmCreateEvents = events.events.filter((item) => context.provider.api.events.evm.Executed.is(item.event));

        expect(evmCreateEvents.length).to.closeTo(275, 10);
    });
});
