import { expect } from "chai";
import { step } from "mocha-steps";
import { describeWithAcala } from "./util";
import { BodhiSigner } from "@acala-network/bodhi";
import { submitExtrinsic } from "./util";
import { BigNumber } from "ethers";

describeWithAcala("Acala RPC (EVM fill block)", (context) => {
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

        const creates = Array(15).fill(context.provider.api.tx.evm.create(
            contract,
            0,
            2_000_000,
            100_000,
            []
        ));

        const tx = context.provider.api.tx.utility.batchAll(creates);
        await submitExtrinsic(tx, alice.substrateAddress);
    });

    step("evm call fill block", async function () {
        // transfer 100000000000 ACA to 0x1000000000000000000000000000000000000001
        const input = '0xa9059cbb000000000000000000000000100000000000000000000000000000000000000100000000000000000000000000000000 0000000000000000000000174876e800';
        const transfers = Array(194).fill(context.provider.api.tx.evm.call(
            '0x0000000000000000000100000000000000000000',
            '0xa9059cbb00000000',
            0,
            100000,
            100000,
            []
        ));

        const batch = context.provider.api.tx.utility.batchAll(transfers);
        await submitExtrinsic(batch, alice.substrateAddress);
    });

    step("evm gas limit", async function () {
        /*
        pragma solidity ^0.8.0;
        contract Factory {
            Contract[] newContracts;
            uint value;
            function createContractLoop (uint count) public {
                for(uint i = 0; i < count; i++) {
                    Contract newContract = new Contract();
                    newContracts.push(newContract);
                }
            }
            function incrementLoop (uint count) public {
                for(uint i = 0; i < count; i++) {
                    value += 1;
                }
            }
        }
        contract Contract {}
        */

        const contract = "0x608060405234801561001057600080fd5b50610335806100206000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80633f8308e61461003b578063659aaab314610057575b600080fd5b61005560048036038101906100509190610182565b610073565b005b610071600480360381019061006c9190610182565b6100ae565b005b60005b818110156100aa57600180600082825461009091906101af565b9250508190555080806100a29061020f565b915050610076565b5050565b60005b8181101561015d5760006040516100c790610161565b604051809103906000f0801580156100e3573d6000803e3d6000fd5b5090506000819080600181540180825580915050600190039060005260206000200160009091909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505080806101559061020f565b9150506100b1565b5050565b605c806102a483390190565b60008135905061017c8161028c565b92915050565b60006020828403121561019857610197610287565b5b60006101a68482850161016d565b91505092915050565b60006101ba82610205565b91506101c583610205565b9250827fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff038211156101fa576101f9610258565b5b828201905092915050565b6000819050919050565b600061021a82610205565b91507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82141561024d5761024c610258565b5b600182019050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b600080fd5b61029581610205565b81146102a057600080fd5b5056fe6080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea264697066735822122003981c658c4f81879e8a61dac66895b300ed8c1522a2d242522caddab6fe5b6464736f6c63430008070033a264697066735822122047d51951d1cde00ab7c772ef239b4d5614518dc107414ff90f297239ff62848f64736f6c63430008070033"

        const tx1 = context.provider.api.tx.evm.create(
            contract, 
            0, 
            2_000_000, 
            100_000, 
            []
        );
        await submitExtrinsic(tx1, alice.substrateAddress);

        const createEvent = (await context.provider.api.query.system.events()).find((record) => record.event.section === 'evm' && record.event.method === 'Created');

        expect(createEvent).to.not.be.undefined;

        const contractAddress = createEvent?.event.data[1]?.toString() || '';
        expect(contractAddress).to.not.be.empty;

        const tx2 = context.provider.api.tx.evm.publishContract(
            contractAddress
        )
        await submitExtrinsic(tx2, alice.substrateAddress);

        const contract_account = await context.provider.api.query.evm.accounts(contractAddress);
        expect(contract_account.unwrap().nonce.toNumber()).to.equal(1);
        expect(contract_account.unwrap().contractInfo.unwrap().published.toString()).to.equal('true');

        // createContractLoop(uint256) 410 times
	    let input1 = "0x659aaab3000000000000000000000000000000000000000000000000000000000000019a";
        const tx3 = context.provider.api.tx.evm.call(
            contractAddress,
            input1,
            0,
            29_000_000,
            5_000_000,
            []
        );
        await submitExtrinsic(tx3, alice.substrateAddress);

        const callEvent1 = (await context.provider.api.query.system.events()).find((record) => record.event.section === 'evm' && record.event.method === 'Executed');
        expect(callEvent1).to.not.be.undefined;

        // incrementLoop(uint256) 8480 times
        let input2 = "0x659aaab3000000000000000000000000000000000000000000000000000000000000019a";
        const tx4 = context.provider.api.tx.evm.call(
            contractAddress,
            input2,
            0,
            29_000_000,
            5_000_000,
            []
        );
        await submitExtrinsic(tx4, alice.substrateAddress);

        const callEvent2 = (await context.provider.api.query.system.events()).find((record) => record.event.section === 'evm' && record.event.method === 'Executed');
        expect(callEvent2).to.not.be.undefined;
    });
});
