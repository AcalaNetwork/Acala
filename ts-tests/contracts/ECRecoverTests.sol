// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.2;

contract ECRecoverTests {
    function ecrecoverTest(bytes memory input) public returns(bytes memory) {
        address ecrecoverAddress = address(0x0000000000000000000000000000000000000001);
        (bool success, bytes memory returnData) = ecrecoverAddress.call(input);

        require(success, "ecrecover address failed");
        return returnData;
    }
}
