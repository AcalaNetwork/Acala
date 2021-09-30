// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity 0.8.2;

contract Block {
    function multiply(uint a) public pure returns(uint d) {
        return a * 7;
    }
    function gasLimit() public view returns(uint) {
        return block.gaslimit;
    }
    function currentBlock() public view returns(uint) {
        return block.number;
    }
    function blockHash(uint number) public view returns(bytes32) {
        return blockhash(number);
    }
    function chainId() public view returns(uint) {
        return block.chainid;
    }
    function coinbase() public view returns(address) {
        return block.coinbase;
    }
    function timestamp() public view returns(uint) {
        return block.timestamp;
    }
}
