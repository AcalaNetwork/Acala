// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.2;

contract ExplicitRevertReason {
    function max10(uint256 a) public pure returns (uint256) {
        if (a > 10) revert("Value must not be greater than 10.");
        return a;
     }
}
