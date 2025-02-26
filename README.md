# Via Network: Bitcoin layer 2 solution

[![Banner](viaBanner.png)](https://onvia.org/)

Via is the Bitcoin layer 2 solution leveraging Bitcoin’s security while enabling scalable off-chain execution. By
utilising modular ZK Stack framework, Via network provides an EVM-compatible execution environment, allowing EVM
developers to deploy and execute smart contracts, while enforcing the transactions integrity via Zero-Knowlegde (ZK)
Proofs.

In Ethereum terms, Via is a Validium that extends Bitcoin’s capabilities by introducing an off-chain execution layer
where transactions are processed efficiently before being finalized on Bitcoin, while the transaction data is stored on
Celestia. This allows for improved throughput and reduced fees while maintaining cryptographic guarantees with Validity
Proofs.

## Knowledge Index

The following questions are addressed in the resources below:

| Question                                                | Resource                                         |
| ------------------------------------------------------- | ------------------------------------------------ |
| What do I need to develop the project locally?          | [development.md](docs/via_guides/development.md) |
| How can I set up my dev environment?                    | [setup-dev.md](docs/guides/setup-dev.md)         |
| How can I run the project?                              | [launch.md](docs/guides/launch.md)               |
| What is the logical project structure and architecture? | [architecture.md](docs/guides/architecture.md)   |
| Via Network Guides                                      | [guides](docs/via_guides/)                       |
| Where can I find protocol specs?                        | Ping Via Team Members                            |
| Where can I find developer docs?                        | Ping Via Team Members                            |

## High Level Overview

![High Level Architecture](viaArchitecture.png)

This repository will contain code for the following components:

- Sequencer
- Proposer
- Prover
- Prover Gateway
- Verifier Network Node

`/core/bin` will contain the binaries for the above components with prefix `via_` e.g. `via_server` for Sequencer and
Proposer software.

Prover and Prover Gateway related code is in the directory `/prover`.

Verifier Network Node implementation can be found in the `/via_verifier` directory.

## Branches

- `main` is the main branch for the project. the code in this branch is the most stable and is used for production.
- `zksync-main`: this branch is equivalent to the zksync-era repo `main` branch.
- (feat/fix/chore)/`<branch-name>`: these branches are used for development and are merged into the `main` branch.
- release/`<version>`: these branches are used for release based on the `main` branch.

> Since we like to be updated with the latest changes in the zksync repo, we will periodically sync the `zksync-main`
> branch with the zksync repo and then merge the changes into the `main` branch. (rebase)

> We also adopt an approach to reduce the possibility of merge conflicts by adding a `via_` prefix to services and
> components that we add to the project and also creating our own new orchestration layer (binaries) for Via project.

```
git remote add upstream git@github.com:matter-labs/zksync-era.git
git checkout zksync-main
git pull upstream main
git checkout main
git rebase zksync-main
```

> This approach will change our git history, so we will need to force push to the `main` branch after the rebase. Please
> be careful when using this approach and communicate with the team before doing so.

## Disclaimer

The Via Protocol is under development and has not been audited yet. Please use the project or code of the project at
your own risk.
