// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.2;

contract TestCalls {
    function test_call(address target, bytes memory input, bytes memory output) public {
        (bool success, bytes memory returnData) = target.call(input);
        assembly {
            if eq(success, 0) {
                revert(add(returnData, 0x20), returndatasize())
            }
        }
        require(keccak256(abi.encodePacked(returnData)) == keccak256(abi.encodePacked(output)), "call reverted");
    }

    function test_static_call(address target, bytes memory input) public view returns(bytes memory) {
        (bool success, bytes memory returnData) = target.staticcall(input);
       assembly {
            if eq(success, 0) {
                revert(add(returnData, 0x20), returndatasize())
            }
        }
        return returnData;
    }

    function test_delegate_call(address target, bytes memory input, bytes memory output) public {
        (bool success, bytes memory returnData) = target.delegatecall(input);
       assembly {
            if eq(success, 0) {
                revert(add(returnData, 0x20), returndatasize())
            }
        }
        require(keccak256(abi.encodePacked(returnData)) == keccak256(abi.encodePacked(output)), "call reverted");
    }
}
