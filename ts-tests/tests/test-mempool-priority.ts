import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import { submitExtrinsic } from "./util";
import { BigNumber } from "ethers";

describeWithAcala("Acala RPC (Mempool Priority Order)", (context) => {
    let alice: BodhiSigner;
    let alice_stash: BodhiSigner;

    const FixedU128 = BigNumber.from('1000000000000000000');

    before("init wallets", async function () {
        [alice, alice_stash] = context.wallets;
    });

    step("transaction pool priority order is correct", async function () {
        const interestRatePerSec = BigNumber.from('10').mul(FixedU128).div(BigNumber.from('100000')).toBigInt();
        const liquidationRatio = BigNumber.from('3').mul(FixedU128).div(BigNumber.from('2')).toBigInt();
        const liquidationPenalty = BigNumber.from('2').mul(FixedU128).div(BigNumber.from('10')).toBigInt();
        const requiredCollateralRatio = BigNumber.from('9').mul(FixedU128).div(BigNumber.from('5')).toBigInt();
        const maximumTotalDebitValue = BigNumber.from("10000000000000000").toBigInt();

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
        await submitExtrinsic(tx1, alice.substrateAddress);

        // change the ACA price
        const tx2 = context.provider.api.tx.sudo.sudo(
            context.provider.api.tx.acalaOracle.feedValues(
                [
                    [{ Token: 'ACA' }, FixedU128.div(BigNumber.from('10')).toBigInt()] // 0.1 USD
                ]
            )
        );
        await submitExtrinsic(tx2, alice.substrateAddress);

        const nonce = (await context.provider.api.rpc.system.accountNextIndex(alice.substrateAddress)).toNumber() + 1;
        const parentHash = await context.provider.api.rpc.chain.getBlockHash();

        // send operational extrinsic
        const tx3 = context.provider.api.tx.sudo.sudo(
            context.provider.api.tx.emergencyShutdown.emergencyShutdown()
        );
        await tx3.signAndSend(alice.substrateAddress, { nonce });

        const operationalTransactionvalidity = await context.provider.api.call.taggedTransactionQueue.validateTransaction(
            "Local",
            tx3.toHex(),
            parentHash
        );
        expect(operationalTransactionvalidity.toHuman()).to.deep.eq({
            "Ok": {
                "longevity": "31",
                "priority": "65,695,198,150,890,000",
                "propagate": true,
                "provides": [
                    "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d04000000"
                ],
                "requires": [
                    "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d03000000"
                ]
            }
        });

        // send normal extrinsic
        const tx4 = context.provider.api.tx.balances.transferAllowDeath(
            alice_stash.substrateAddress,
            80_000
        );
        await tx4.signAndSend(alice.substrateAddress, { nonce });
        const normalTransactionvalidity = await context.provider.api.call.taggedTransactionQueue.validateTransaction(
            "Local",
            tx4.toHex(),
            parentHash
        );
        expect(normalTransactionvalidity.toHuman()).to.deep.eq({
            "Ok": {
                "longevity": "31",
                "priority": "0",
                "propagate": true,
                "provides": [
                    "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d04000000"
                ],
                "requires": [
                    "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d03000000"
                ]
            }
        });

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
        expect(unsignedTransactionvalidity.toHuman()).to.deep.eq({
            "Ok": {
                "longevity": "64",
                "priority": "14,999,999,999,000",
                "propagate": true,
                "provides": [
                    "0x5c434450456e67696e654f6666636861696e576f726b657207000000000000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
                ],
                "requires": []
            }
        });

        // Ensure tx priority order:
	    // Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
        expect(operationalTransactionvalidity.asOk.priority > unsignedTransactionvalidity.asOk.priority).to.be.true;
        expect(unsignedTransactionvalidity.asOk.priority > normalTransactionvalidity.asOk.priority).to.be.true;
    });
});
