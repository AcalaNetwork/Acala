// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.0;

contract CreateContractFactory {
    event newContract(address indexed newContract);

    ParentContract[] public contracts;
    function createContract () public {
        ParentContract parentContract = new ParentContract();
        emit newContract(address(parentContract));

        contracts.push(parentContract);
    }

    function callContract () public {
        require(contracts.length > 0, "Need to create contract");

        ParentContract(contracts[0]).createChild();
    }
}
contract ParentContract {
    event newChildContract(address indexed newChildContract);

    ChildContract public childContract;
    ChildContract[] public childContracts;

    constructor() {
        childContract = new ChildContract();
        emit newChildContract(address(childContract));
    }

    function createChild() public {
        ChildContract _childContract = new ChildContract();
        emit newChildContract(address(_childContract));

        childContracts.push(_childContract);
    }
}

contract ChildContract {
    uint public time;
    constructor() {
        time = block.timestamp;
    }
}
