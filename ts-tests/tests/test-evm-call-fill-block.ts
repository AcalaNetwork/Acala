import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import { submitExtrinsic } from "./util";
import { BigNumber } from "ethers";

describeWithAcala("Acala RPC (EVM call fill block)", (context) => {
    let alice: BodhiSigner;
    let alice_stash: BodhiSigner;

    const FixedU128 = BigNumber.from('1000000000000000000');

    before("init wallets", async function () {
        [alice, alice_stash] = context.wallets;
    });

    step("evm create fill block", async function () {
        /*
        pragma solidity ^0.8.0;
        contract Contract {}
        */

        const contract = "0x6080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea2646970667358221220b9cbc7f3d9528c236f2c6bdf64e25ac8ca17489f9b4e91a6d92bea793883d5d764736f6c63430008020033";

        const creates = Array(300).fill(context.provider.api.tx.evm.create(
            contract,
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

            setTimeout(() => { }, 1000);
        }

        let currentBlockHash = await context.provider.api.rpc.chain.getBlockHash(beforeHeight + 1);

        const events = await context.provider.api.derive.tx.events(currentBlockHash);

        const evmCreateEvents = events.events.filter((item) => context.provider.api.events.evm.Created.is(item.event));

        expect(evmCreateEvents.length).to.equal(223);
    });
});
