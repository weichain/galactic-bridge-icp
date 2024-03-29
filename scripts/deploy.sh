#!/bin/bash

# Default values
NETWORK="local"
CREATE_MINTER=false
CREATE_LEDGER=false

# Parse command-line options
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --reinstall)
            MODE="reinstall"
            ;;
        -n | --network)
            if [[ -n "$2" ]]; then
                NETWORK="$2"
                shift
            else
                echo "Error: Network option requires a value."
                exit 1
            fi
            ;;
        --all)
            CREATE_MINTER=true
            CREATE_LEDGER=true
            ;;
        --minter)
            CREATE_MINTER=true
            ;;
        --ledger)
            CREATE_LEDGER=true
            ;;
        *) 
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
    shift
done

# create canisters
dfx canister create ledger --network=$NETWORK --identity=$DEPLOYER_PRINCIPAL_NAME
dfx canister create minter --network=$NETWORK --identity=$DEPLOYER_PRINCIPAL_NAME


# Construct the arguments for dfx deploy command
if $CREATE_LEDGER; then
  LEDGER_ARGUMENTS=(
    ledger
    --argument "
      (variant {
        Init = record {
          token_name = \"ICP Solana\";
          token_symbol = \"ckSol\";
          minting_account = record {
            owner = principal \"$(dfx canister id minter --network="$NETWORK")\";
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
    --network="$NETWORK"
    --identity="$DEPLOYER_PRINCIPAL_NAME"
    --upgrade-unchanged
  )

  # Add --mode argument if --reinstall flag is present
  if [[ "$MODE" == "reinstall" ]]; then
    LEDGER_ARGUMENTS+=(--mode "$MODE")
  fi

  # Deploy ledger canister
  dfx deploy "${LEDGER_ARGUMENTS[@]}" 
fi 

# Construct the arguments for dfx deploy command
if $CREATE_MINTER; then
  MINTER_ARGUMENTS=(
    minter
    --argument "
      (variant {
        Init = record {
          solana_network = variant { Testnet };
          solana_contract_address = \"7PkxG43UrwKPtET5TBhgT4YZwsGr8R77j9CVrroDqg5m\";
          solana_initial_signature = \"2mKYvViaoudLNVPQ4Bggs3nCfQJyFoTo9gCAC7gYBnrp3t8VqQnEyCmb5W3vzcgsXVH4PfLFnMZoTAq1cwuKM4g6\";
          ecdsa_key_name = \"dfx_test_key\";
          ledger_id = principal \"$(dfx canister id ledger --network="$NETWORK")\";
          minimum_withdrawal_amount = 100;
        }
      })
    "
    --network="$NETWORK"
    --identity="$DEPLOYER_PRINCIPAL_NAME"
    --upgrade-unchanged
  )

  # Add --mode argument if --reinstall flag is present
  if [[ "$MODE" == "reinstall" ]]; then
    MINTER_ARGUMENTS+=(--mode "$MODE")
  fi

  # Deploy minter canister
  dfx deploy "${MINTER_ARGUMENTS[@]}" 
fi