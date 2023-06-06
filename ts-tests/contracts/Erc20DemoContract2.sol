// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.2;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract Erc20DemoContract2 is ERC20 {
    constructor() ERC20("long string name, long string name, long string name, long string name, long string name", "TestToken") {
        // mint alice 100_000_000_000_000_000_000_000
        _mint(0x1000000000000000000000000000000000000001, 100_000_000_000_000_000_000_000);
    }

    function decimals() public view virtual override returns (uint8) {
        return 17;
    }
}
