import { task } from 'hardhat/config';
import {} from '../../typechain-types';
import * as fs from 'fs';
import { Wallet } from 'zksync-ethers';
import { getProvider } from './utils';
import { L2BaseTokenFactory } from '../../typechain/L2BaseTokenFactory';
import { L2BaseToken } from '../../typechain/L2BaseToken';
import { ethers } from 'ethers';

task('withdrawbtc', 'withdrawl tokens to L1')
    .addParam('amount', 'The amount of BTC to send')
    .setAction(async (taskArgs, hre) => {
        const provider = getProvider(hre.network.config.url, hre.network.name);
        const wallet = new Wallet(process.env.PK!, provider);
        const { amount } = taskArgs;
        const btcAddress = ethers.hexlify(ethers.toUtf8Bytes('bcrt1qx2lk0unukm80qmepjp49hwf9z6xnz0s73k9j56'));

        const contract = L2BaseTokenFactory.connect(
            '0x000000000000000000000000000000000000800a',
            wallet
        ) as L2BaseToken;
        const value = amount;
        console.log(await contract.balanceOf(wallet.address));
        await contract.connect(wallet).withdraw(btcAddress, { value: '1000000' });
        console.log(await contract.balanceOf(wallet.address));
        // await tx.wait();

        console.log('Contributed amount:', value);
    });

export default {};

// 0x0968f2640000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002c62637274317178326c6b30756e756b6d3830716d65706a703439687766397a36786e7a307337336b396a35360000000000000000000000000000000000000000
