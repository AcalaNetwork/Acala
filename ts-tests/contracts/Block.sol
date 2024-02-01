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
    function difficulty() public view returns(uint) {
        return block.difficulty;
    }
    function gas_limit() public view returns(uint) {
        return block.gas_limit;
    }
    function randomness() public view returns(uint) {
        return block.randomness;
    }

    //std::map<std::string, Type const*> const txVars{
	// 	{"block.basefee", TypeProvider::uint256()},
	// 	{"block.chainid", TypeProvider::uint256()},
	// 	{"block.coinbase", TypeProvider::address()},
	// 	{"block.prevrandao", TypeProvider::uint256()},
	// 	{"block.gaslimit", TypeProvider::uint256()},
	// 	{"block.number", TypeProvider::uint256()},
	// 	{"block.timestamp", TypeProvider::uint256()},
	// 	{"blockhash", TypeProvider::array(DataLocation::Memory, TypeProvider::uint256())},
	// 	{"msg.data", TypeProvider::array(DataLocation::CallData)},
	// 	{"msg.sender", TypeProvider::address()},
	// 	{"msg.sig", TypeProvider::fixedBytes(4)},
	// 	{"msg.value", TypeProvider::uint256()},
	// 	{"tx.gasprice", TypeProvider::uint256()},
	// 	{"tx.origin", TypeProvider::address()}
	// };
}
