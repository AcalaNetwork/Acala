import { expect, beforeAll, it } from "vitest";
import { describeWithAcala, transfer } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";

describeWithAcala("Acala RPC (Balance)", (context) => {
    let alice: BodhiSigner;
    let alice_stash: BodhiSigner;

    beforeAll(async function () {
        [alice, alice_stash] = context.wallets;
    });

    it("genesis balance is setup correctly", async function () {
        expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999995072568421000000");
        expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString()).to.equal("8999999995072568421000000");

        expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString())
            .to.equal((await context.provider.api.query.system.account(alice.substrateAddress)).data.free.toString() + "000000");
    });

    it("balance to be updated after transfer", async function () {
        expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999995072568421000000");
        expect((await context.provider.getBalance(alice_stash.getAddress())).toString()).to.equal("10100000995072575531000000");

        await transfer(context, alice.substrateAddress, alice_stash.substrateAddress, 1000);
        expect((await context.provider.getBalance(alice.getAddress())).toString()).to.equal("8999999990446773722000000");
        expect((await context.provider.getBalance(alice_stash.getAddress())).toString()).to.equal("10100000995072576531000000");
        expect((await context.provider.getBalance(alice.getAddress(), "latest")).toString()).to.equal("8999999990446773722000000");
        expect((await context.provider.getBalance(alice_stash.getAddress(), "earliest")).toString()).to.equal("0");
    });
});
