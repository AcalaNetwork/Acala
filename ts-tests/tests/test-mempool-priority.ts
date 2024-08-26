import { expect, beforeAll, it } from "vitest";
import { describeWithAcala, submitExtrinsic } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import { BigNumber } from "ethers";
import { BuildBlockMode } from "@acala-network/chopsticks";

describeWithAcala("Acala RPC (Mempool Priority Order)", (context) => {
    let alice: BodhiSigner;
    let alice_stash: BodhiSigner;

    const FixedU128 = BigNumber.from('1000000000000000000');

    beforeAll(async function () {
        [alice, alice_stash] = context.wallets;
    });

    it("transaction pool priority order is correct", async function () {
        const interestRatePerSec = BigNumber.from('10').mul(FixedU128).div(BigNumber.from('100000')).toBigInt();
        const liquidationRatio = BigNumber.from('3').mul(FixedU128).div(BigNumber.from('2')).toBigInt();
        const liquidationPenalty = BigNumber.from('2').mul(FixedU128).div(BigNumber.from('10')).toBigInt();
        const requiredCollateralRatio = BigNumber.from('9').mul(FixedU128).div(BigNumber.from('5')).toBigInt();
        const maximumTotalDebitValue = BigNumber.from("10000000000000000").toBigInt();

        const nonce = (await context.provider.api.query.system.account(alice.substrateAddress)).nonce.toNumber();

        // setup an unsafe cdp
        const tx1 = context.provider.api.tx.utility.batchAll([
            context.provider.api.tx.sudo.sudo(
                context.provider.api.tx.acalaOracle.feedValues(
                    [
                        [{ Token: 'ACA' }, BigNumber.from('1').mul(FixedU128).toString()] // 1 USD
                    ]
                )
            ),
            context.provider.api.tx.sudo.sudo(context.provider.api.tx.cdpEngine.setCollateralParams(
                { Token: 'ACA' },
                { NewValue: interestRatePerSec },
                { NewValue: liquidationRatio },
                { NewValue: liquidationPenalty },
                { NewValue: requiredCollateralRatio },
                { NewValue: maximumTotalDebitValue }
            )),
            context.provider.api.tx.honzon.adjustLoan(
                { Token: 'ACA' }, // currency_id
                100000000000000, // collateral_adjustment
                500000000000000 // debit_adjustment
            )
        ]);
        await submitExtrinsic(tx1, alice.substrateAddress, nonce);

        // change the ACA price
        const tx2 = context.provider.api.tx.sudo.sudo(
            context.provider.api.tx.acalaOracle.feedValues(
                [
                    [{ Token: 'ACA' }, FixedU128.div(BigNumber.from('10')).toBigInt()] // 0.1 USD
                ]
            )
        );
        await submitExtrinsic(tx2, alice.substrateAddress, nonce + 1);

        context.chain.txPool.mode = BuildBlockMode.Manual;

        const parentHash = await context.provider.api.rpc.chain.getBlockHash();

        // send operational extrinsic
        const tx3 = context.provider.api.tx.sudo.sudo(
            context.provider.api.tx.emergencyShutdown.emergencyShutdown()
        );
        await tx3.signAsync(alice.substrateAddress, { nonce: nonce + 2 });

        const operationalTransactionvalidity = await context.provider.api.call.taggedTransactionQueue.validateTransaction(
            "Local",
            tx3.toHex(),
            parentHash
        );

        expect(operationalTransactionvalidity).toMatchInlineSnapshot(`
          {
            "ok": {
              "longevity": 31,
              "priority": "0x0119dfa51d01f600",
              "propagate": true,
              "provides": [
                "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d03000000",
              ],
              "requires": [],
            },
          }
        `);

        // send normal extrinsic
        const tx4 = context.provider.api.tx.balances.transferAllowDeath(
            alice_stash.substrateAddress,
            80_000
        );
        await tx4.signAsync(alice.substrateAddress, { nonce: nonce + 2 });
        const normalTransactionvalidity = await context.provider.api.call.taggedTransactionQueue.validateTransaction(
            "Local",
            tx4.toHex(),
            parentHash
        );
        expect(normalTransactionvalidity.toHuman()).toMatchInlineSnapshot(`
          {
            "Ok": {
              "longevity": "31",
              "priority": "0",
              "propagate": true,
              "provides": [
                "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d03000000",
              ],
              "requires": [],
            },
          }
        `);

        // send unsigned extrinsic
        const tx5 = context.provider.api.tx.cdpEngine.liquidate(
            { Token: 'ACA' }, // currency_id
            alice.substrateAddress, // target
        );
        const unsignedTransactionvalidity = await context.provider.api.call.taggedTransactionQueue.validateTransaction(
            "Local",
            tx5.toHex(),
            parentHash
        );

        expect(unsignedTransactionvalidity).toMatchInlineSnapshot(`
          {
            "ok": {
              "longevity": 64,
              "priority": 14999999999000,
              "propagate": true,
              "provides": [
                "0x5c434450456e67696e654f6666636861696e576f726b657208000000000000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",
              ],
              "requires": [],
            },
          }
        `);

        // Ensure tx priority order:
        // Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
        expect(operationalTransactionvalidity.asOk.priority > unsignedTransactionvalidity.asOk.priority).to.be.true;
        expect(unsignedTransactionvalidity.asOk.priority > normalTransactionvalidity.asOk.priority).to.be.true;
    });
});
