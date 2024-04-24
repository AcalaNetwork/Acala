// SPDX-License-Identifier: GPL-3.0-or-later

pragma solidity ^0.8.2;

contract Block {
    function multiply(uint a) public pure returns(uint d) {
        return a * 7;
    }

    function baseFee() public view returns(uint) {
        return block.basefee;
    }
    function chainId() public view returns(uint) {
        return block.chainid;
    }
    function coinbase() public view returns(address) {
        return block.coinbase;
    }
    function prevrandao() public view returns(uint) {
        return block.prevrandao;
    }
    function gasLimit() public view returns(uint) {
        return block.gaslimit;
    }
    function blockNumber() public view returns(uint) {
        return block.number;
    }
    function timestamp() public view returns(uint) {
        return block.timestamp;
    }
    function blockHash(uint number) public view returns(bytes32) {
        return blockhash(number);
    }
}
