import * as utils from 'utils';
import { Command } from 'commander';
import { Wallet, Provider, Signer, Contract } from 'zksync-ethers';
import { ethers } from 'ethers';

const DEFAULT_DEPOSITOR_PRIVATE_KEY = 'cVZduZu265sWeAqFYygoDEE1FZ7wV9rpW5qdqjRkUehjaUMWLT1R';
const DEFAULT_NETWORK = 'regtest';
const DEFAULT_RPC_URL = 'http://0.0.0.0:18443';
const DEFAULT_RPC_USERNAME = 'rpcuser';
const DEFAULT_RPC_PASSWORD = 'rpcpassword';

// 0x36615Cf349d7F6344891B1e7CA7C72883F5dc049
const DEFAULT_L2_PRIVATE_KEY = '0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110';
const DEFAULT_L2_RPC_URL = 'http://0.0.0.0:3050';
const L2_BASE_TOKEN_SYSTEM_CONTRACT_ADDR = '000000000000000000000000000000000000800a';

async function verifyBatch(batchProofRefRevealTxId: string) {
    process.chdir(`${process.env.VIA_HOME}`);
    await utils.spawn(`cargo run --example verify_batch -- ${batchProofRefRevealTxId}`);
}

async function deposit(
    amount: number,
    receiverL2Address: string,
    senderPrivateKey: string,
    network: String,
    rcpUrl: string,
    rpcUsername: string,
    rpcPassword: string
) {
    if (isNaN(amount)) {
        console.error('Error: Invalid deposit amount. Please provide a valid number.');
        return;
    }
    process.chdir(`${process.env.VIA_HOME}`);
    await utils.spawn(
        `cargo run --example deposit -- ${amount} ${receiverL2Address} ${senderPrivateKey} ${network} ${rcpUrl} ${rpcUsername} ${rpcPassword}`
    );
}

async function withdraw(amount: number, receiverL1Address: string, userPrivateKey: string, rcpUrl: string) {
    if (isNaN(amount)) {
        console.error('Error: Invalid withdraw amount. Please provide a valid number.');
        return;
    }

    const abi = [
        {
            inputs: [
                {
                    internalType: 'bytes',
                    name: '_l1Receiver',
                    type: 'bytes'
                }
            ],
            name: 'withdraw',
            outputs: [],
            stateMutability: 'payable',
            type: 'function'
        }
    ];
}

export const command = new Command('verifier').description('verifier network mock');

command
    .command('verify-batch')
    .description('verify batch by batch da ref reveal tx id')
    .requiredOption(
        '--batch-proof-ref-reveal-tx-id <batchProofRefRevealTxId>',
        'reveal tx id for the l1 batch proof to verify'
    )
    .action((cmd: Command) => verifyBatch(cmd.batchProofRefRevealTxId));

command
    .command('deposit')
    .description('deposit BTC to l2')
    .requiredOption('--amount <amount>', 'amount of BTC to deposit', parseFloat)
    .requiredOption('--receiver-l2-address <receiverL2Address>', 'receiver l2 address')
    .option('--sender-private-key <senderPrivateKey>', 'sender private key', DEFAULT_DEPOSITOR_PRIVATE_KEY)
    .option('--network <network>', 'network', DEFAULT_NETWORK)
    .option('--rpc-url <rcpUrl>', 'RPC URL', DEFAULT_RPC_URL)
    .option('--rpc-username <rcpUsername>', 'RPC username', DEFAULT_RPC_USERNAME)
    .option('--rpc-password <rpcPassword>', 'RPC password', DEFAULT_RPC_PASSWORD)
    .action((cmd: Command) =>
        deposit(
            cmd.amount,
            cmd.receiverL2Address,
            cmd.senderPrivateKey,
            cmd.network,
            cmd.rpcUrl,
            cmd.rpcUsername,
            cmd.rpcPassword
        )
    );

command
    .command('withdraw')
    .description('withdraw BTC to l1')
    .requiredOption('--amount <amount>', 'amount of BTC to withdraw', parseFloat)
    .requiredOption('--receiver-l1-address <receiverL1Address>', 'receiver l1 address')
    .option('--user-private-key <userPrivateKey>', 'user private key', DEFAULT_L2_PRIVATE_KEY)
    .option('--rpc-url <rcpUrl>', 'RPC URL', DEFAULT_L2_RPC_URL)
    .action((cmd: Command) => withdraw(cmd.amount, cmd.receiverL1Address, cmd.userPrivateKey, cmd.rpcUrl));
