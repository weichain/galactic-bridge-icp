#!/bin/bash

# dfx canister create ledger
dfx canister create minter

# dfx deploy ledger --upgrade-unchanged --argument "(variant {
#   Init = record {
#     minting_account = record {
#       owner = principal \"$OWNER_PRINCIPAL\"
#     };
#     feature_flags  = opt record { icrc2 = true };
#     decimals = opt 18;
#     max_memo_length = opt 80;
#     transfer_fee = 0;
#     token_symbol = \"ckSol\";
#     token_name = \"Chain key Solana\";
#     metadata = vec {};
#     initial_balances = vec {};
#     archive_options = record {
#       num_blocks_to_archive = 1000;
#       trigger_threshold = 2000; 
#       max_message_size_bytes = null;
#       cycles_for_archive_creation = opt 1_000_000_000_000;
#       node_max_memory_size_bytes = opt 3_221_225_472;
#       controller_id = principal \"$OWNER_PRINCIPAL\";
#     }
#   }
# })"


dfx deploy minter --argument '(variant {
  InitArg = record {
    ethereum_network = variant {Sepolia};
    ecdsa_key_name = "key_1";
    ethereum_contract_address = opt "0xb44B5e756A894775FC32EDdf3314Bb1B1944dC34";
    ledger_id = principal "'"$(dfx canister id ledger)"'";
    ethereum_block_height = variant {Finalized};
    minimum_withdrawal_amount = 10_000_000_000_000_000;
    next_transaction_nonce = 1;
    last_scraped_block_number = 1
  }
})'