#!/bin/bash

# create canisters
dfx canister create ledger
dfx canister create minter

# deploy collateral_token canister
dfx deploy ledger --upgrade-unchanged --argument "
  (variant {
    Init = record {
      token_name = \"ICP Solana\";
      token_symbol = \"ckSol\";
      minting_account = record {
        owner = principal \"$(dfx canister id minter)\";
      };
      initial_balances = vec {};
      metadata = vec {};
      transfer_fee = 0;
      archive_options = record {
        trigger_threshold = 2000;
        num_blocks_to_archive = 1000;
        controller_id = principal \"$OWNER_PRINCIPAL\";
      };
      feature_flags = opt record {
        icrc2 = true;
      };
    }
  })
"

dfx deploy minter --upgrade-unchanged  --argument "
  (variant {
    Init = record {
      solana_network = variant { Testnet };
      solana_contract_address = \"asd\";
      ecdsa_key_name = \"asd\";
      ledger_id = principal \"$(dfx canister id ledger)\";
      minimum_withdrawal_amount = 100;
    }
  })
"