// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.2;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract Erc20DemoContract is ERC20 {
    constructor(uint256 initialSupply) ERC20("long string name, long string name, long string name, long string name, long string name", "TestToken") {
        // mint msg.sender initialSupply
        _mint(msg.sender, initialSupply);
    }

    function decimals() public view virtual override returns (uint8) {
        return 17;
    }
}
