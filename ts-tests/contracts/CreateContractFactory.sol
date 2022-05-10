// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.0;

contract CreateContractFactory {
    event newContract(address indexed newContract);

    Contract[] public Contracts;
    function createContract () public {
        Contract _newContract = new Contract();
        emit newContract(address(_newContract));

        Contracts.push(_newContract);
    }
}
contract Contract {
    event newChildContract(address indexed newChildContract);

    ChildContract public childContract;
    constructor() {
        childContract = new ChildContract();
        emit newChildContract(address(childContract));
    }
}

contract ChildContract {
    uint public time;
    constructor() {
        time = block.timestamp;
    }
}
