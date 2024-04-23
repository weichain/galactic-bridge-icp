#!/bin/bash

# deploy minter canister
dfx deploy minter --upgrade-unchanged --argument "
  (variant {
    Upgrade = record {
      solana_rpc_url = opt \"\";
    }
  })
" --yes --identity="$OWNER_PRINCIPAL_NAME"
