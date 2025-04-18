import { task } from 'hardhat/config';
import { CrowdfundingCampaign, CrowdfundingCampaign__factory } from '../../typechain-types';
import * as fs from 'fs';
import { Wallet } from 'zksync-ethers';
import { getProvider } from './utils';

task('stats', 'Get crowdfunding stats').setAction(async (taskArgs, hre) => {
    const provider = getProvider(hre.network.config.url, hre.network.name);
    const wallet = new Wallet(process.env.PK!, provider);

    const config: any = JSON.parse(fs.readFileSync('config.json', 'utf-8'));
    const factory = new CrowdfundingCampaign__factory();
    const contract = factory.connect(wallet).attach(config.contract) as CrowdfundingCampaign;
    const raisedAmount = await contract.getTotalFundsRaised();
    const fundingGoal = await contract.getFundingGoal();

    console.log(`crowdfunding stats ${raisedAmount}/${fundingGoal}`);
});

export default {};
