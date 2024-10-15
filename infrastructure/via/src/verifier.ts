import * as utils from 'utils';
import { Command } from 'commander';

const DEFAULT_DEPOSITOR_PRIVATE_KEY = 'cVZduZu265sWeAqFYygoDEE1FZ7wV9rpW5qdqjRkUehjaUMWLT1R';

async function verifyBatch(batchProofRefRevealTxId: string) {
    process.chdir(`${process.env.VIA_HOME}`);
    await utils.spawn(`cargo run --example verify_batch -- ${batchProofRefRevealTxId}`);
}

async function deposit(amount: number, receiverL2Address: string, senderPrivateKey: string) {
    if (isNaN(amount)) {
        console.error('Error: Invalid deposit amount. Please provide a valid number.');
        return;
    }
    process.chdir(`${process.env.VIA_HOME}`);
    await utils.spawn(`cargo run --example deposit -- ${amount} ${receiverL2Address} ${senderPrivateKey}`);
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
    .action((cmd: Command) => deposit(cmd.amount, cmd.receiverL2Address, cmd.senderPrivateKey));
