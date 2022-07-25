// SPDX-License-Identifier: GPL-3.0

pragma solidity ^0.8.2;
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
    
contract MockLiquidationContract {
    event Liquidate(address collateral, address payable repayDest, uint256 supply, uint256 target);
    event OnCollateralTransfer(address collateral, uint256 amount);
    event OnRepaymentRefund(address collateral, uint256 amount);

    address public constant KUSD = 0x0000000000000000000100000000000000000081;
    address public constant AUSD = 0x0000000000000000000100000000000000000001;
    
    function liquidate(address collateral, address payable repayDest, uint256 supply, uint256 target) public {
        if(IERC20(KUSD).balanceOf(address(this)) >= target) {
             IERC20(KUSD).transfer(repayDest, target);
        } else if(IERC20(AUSD).balanceOf(address(this)) >= target) {
             IERC20(AUSD).transfer(repayDest, target);
        }
        
        emit Liquidate(collateral, repayDest, supply, target);
    }
    function onCollateralTransfer(address collateral, uint256 amount) public {
        emit OnCollateralTransfer(collateral, amount);
    }
    function onRepaymentRefund(address collateral, uint256 amount) public {
        emit OnRepaymentRefund(collateral, amount);
    }
}
