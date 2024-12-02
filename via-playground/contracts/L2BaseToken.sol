// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface L2BaseToken {
    function balanceOf(uint256 _account) external view returns (uint256);

    function withdraw(bytes calldata _l1Receiver) external payable;
}
