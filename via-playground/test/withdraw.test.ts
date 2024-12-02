import hre, { ethers } from 'hardhat';
import { expect } from 'chai';
import { getProvider } from '../scripts/tasks/utils';
import { Wallet } from 'zksync-ethers';
import { L2BaseTokenFactory } from '../typechain/L2BaseTokenFactory';
import { L2BaseToken } from '../typechain/L2BaseToken';

describe('MyContract', function () {
    beforeEach(async function () {});

    describe('Deployment', function () {
        it('should deploy successfully', async function () {
            const provider = getProvider(hre.network.config.url, hre.network.name);
            const wallet = new Wallet(process.env.PK!, provider);
            const btcAddress = ethers.hexlify(ethers.toUtf8Bytes('bcrt1qx2lk0unukm80qmepjp49hwf9z6xnz0s73k9j56'));
            console.log(btcAddress);
            const contract = L2BaseTokenFactory.connect(
                '0x000000000000000000000000000000000000800a',
                wallet
            ) as L2BaseToken;
            console.log(await contract.balanceOf(wallet.address));
            const tx = await contract.withdraw(btcAddress, { value: '1000000' });
            console.log(await contract.balanceOf(wallet.address));
            // const tx = await contract.connect(wallet).withdraw(btcAddress, { value: "1000000" });
            // console.log(await contract.balanceOf("0x36615Cf349d7F6344891B1e7CA7C72883F5dc049"));
        });
    });
});
