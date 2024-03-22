#!/bin/bash

# create canisters
dfx canister create ledger
dfx canister create minter

# Deploy ledger
./scripts/deploy_ledger.sh

# Deploy minter
./scripts/deploy_minter.sh