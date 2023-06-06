// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.2;

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
