#!/bin/bash

# deploy minter canister
dfx deploy minter --upgrade-unchanged --argument "
  (variant {
    Upgrade = record {
      ecdsa_key_name = opt \"test_key_1\";
    }
  })
" --yes --network=ic --identity="$DEPLOYER_PRINCIPAL_NAME"
