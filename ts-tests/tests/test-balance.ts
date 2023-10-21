import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala, transfer } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (Balance)", (context) => {
    let alice: BodhiSigner;
    let alice_stash: BodhiSigner;

    before("init wallets", async function () {
        [alice, alice_stash] = context.wallets;
    });

    step("genesis balance is setup correctly", async function () {
        expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999995937426817000000");
        expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString()).to.equal("8999999995937426817000000");

        expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString())
            .to.equal((await context.provider.api.query.system.account(alice.substrateAddress)).data.free.toString() + "000000");
    });

    step("balance to be updated after transfer", async function () {
        this.timeout(15000);

        expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999995937426817000000");
        expect((await context.provider.getBalance(alice_stash.getAddress())).toString()).to.equal("10100000995937431778000000");

        await transfer(context, alice.substrateAddress, alice_stash.substrateAddress, 1000);
        expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999992037566906000000");
        expect((await context.provider.getBalance(alice_stash.getAddress())).toString()).to.equal("10100000995937432778000000");
        expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString()).to.equal("8999999992037566906000000");
        expect((await context.provider.getBalance(alice_stash.getAddress(), "earliest")).toString()).to.equal("0");
    });
});
