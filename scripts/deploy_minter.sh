#!/bin/bash

# deploy minter canister
dfx deploy minter --upgrade-unchanged --mode reinstall --argument "
  (variant {
    Init = record {
      solana_network = variant { Testnet };
      solana_contract_address = \"7PkxG43UrwKPtET5TBhgT4YZwsGr8R77j9CVrroDqg5m\";
      solana_initial_signature = \"5v94qMkugiYWc97VS2DeKCdRbvgHUXis9snBx72gSbqJDsJ2xMdvMGwVs12Z5xsDD9WeEU1pMqzoWLwdbguLVyLA\";
      ecdsa_key_name = \"dfx_test_key\";
      ledger_id = principal \"$(dfx canister id ledger)\";
      minimum_withdrawal_amount = 100;
    }
  })
" --yes

