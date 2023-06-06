// SPDX-License-Identifier: GPL-3.0

pragma solidity ^0.8.2;

contract LiquidationOk {
    event Liquidate(address collateral, address repayDest, uint256 supply, uint256 target);
    event OnCollateralTransfer(address collateral, uint256 amount);
    event OnRepaymentRefund(address collateral, uint256 amount);

    function liquidate(address collateral, address repayDest, uint256 supply, uint256 target) public {
        emit Liquidate(collateral, repayDest, supply, target);
    }
    function onCollateralTransfer(address collateral, uint256 amount) public {
        emit OnCollateralTransfer(collateral, amount);
    }
    function onRepaymentRefund(address collateral, uint256 amount) public {
        emit OnRepaymentRefund(collateral, amount);
    }
}
