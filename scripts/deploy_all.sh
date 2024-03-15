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

dfx deploy minter --upgrade-unchanged --mode reinstall --argument "
  (variant {
    Init = record {
      solana_network = variant { Testnet };
      solana_contract_address = \"HS6NTv6GBVSLct8dsimRWRvjczJTAgfgDJt8VpR8wtGm\";
      solana_initial_signature = \"2qaGQLHQvfnjNonadNk3FQ17tSJQ7rCqSXUckaa7qhJQWGmHYptxW7V98YxFK1sE8NWz8G3nU5b3BzWbJur3ZhU4\";
      ecdsa_key_name = \"dfx_test_key\";
      ledger_id = principal \"$(dfx canister id ledger)\";
      minimum_withdrawal_amount = 100;
    }
  })
" --yes

