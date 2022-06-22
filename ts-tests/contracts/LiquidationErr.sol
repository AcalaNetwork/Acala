// SPDX-License-Identifier: GPL-3.0

pragma solidity ^0.8.2;

contract LiquidationErr {

    function liquidate(address collateral, address repayDest, uint256 supply, uint256 target) public {
        revert("Err");
    }
    function onCollateralTransfer(address collateral, uint256 amount) public {
        revert("Err");
    }
    function onRepaymentRefund(address collateral, uint256 amount) public {
        revert("Err");
    }
}
