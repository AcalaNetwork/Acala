import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import EmptyContract from "../build/EmptyContract.json"

describeWithAcala("Acala RPC (EVM create fill block)", (context) => {
    let alice: BodhiSigner;

    before("init wallets", async function () {
        [alice] = context.wallets;
    });

    step("evm create fill block", async function () {
        const bytecode = '0x' + EmptyContract.bytecode;
        const creates = Array(300).fill(context.provider.api.tx.evm.create(
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

        while (true) {
            const currentHeight = await context.provider.api.query.system.number();

            if (currentHeight.toNumber() > beforeHeight) {
                break;
            }

            await new Promise(resolve => setTimeout(resolve, 1000));
        }

        let currentBlockHash = await context.provider.api.rpc.chain.getBlockHash(beforeHeight + 1);

        const events = await context.provider.api.derive.tx.events(currentBlockHash);

        const evmCreateEvents = events.events.filter((item) => context.provider.api.events.evm.Created.is(item.event));

        expect(evmCreateEvents.length).to.equal(223);
    });
});
