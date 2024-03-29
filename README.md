# galactic_bridge_icp

## Deployment
Deploys ledger and minter

```bash
dfx start --background

./scripts/export.sh
./scripts/deploy.sh --all
```

## (Re)Generating candid file (minter.did)
```bash
./scripts/did.sh
```

# Flow examples

## Sol to ckSol
```
┌────┐ ┌───────────────┐           ┌──────────┐┌──────────┐
│User│ │Solana Contract│           │  Minter  ││  Ledger  │
└─┬──┘ └───────┬───────┘           └────┬─────┘└─────┬────┘
  │            │                        │            │
  │   deposit  │                        │            │
  │───────────>│                        │            │
  │            │   scraps transactions  │            │
  │            │<───────────────────────│            │
  │            │                        │ on deposit │
  │            │                        │ mint asset │
  │            │                        │───────────>│
  │            │                        │            │
┌─┴──┐ ┌───────┴───────┐           ┌────┴─────┐┌─────┴────┐
│User│ │Solana Contract│           │  Minter  ││  Ledger  │
└────┘ └───────────────┘           └──────────┘└──────────┘
```

## ckSol to Sol
```
 ┌───────────────┐ ┌────┐             ┌──────────┐┌──────────┐
 │Solana Contract│ │User│             │  Minter  ││  Ledger  │
 └─┬─────────────┘ └─┬──┘             └────┬─────┘└─────┬────┘
   │                 │                     │            │
   │                 │ give "icrc2_approve" to minter   │
   │                 │─────────────────────────────────>│
   │                 │                     │            │
   │                 │ burn                │            │
   │                 │────────────────────>│transfer_from
   │                 │                     │───────────>│
   │                 │ generate coupon     │            │
   │                 │<────────────────────│            │
   │  withdraw       │                     │            │
   │<────────────────│                     │            │
   │                 │                     │            │
 ┌─┴─────────────┐ ┌─┴──┐             ┌────┴─────┐┌─────┴────┐
 │Solana Contract│ │User│             │  Minter  ││  Ledger  │
 └───────────────┘ └────┘             └──────────┘└──────────┘
```
Coupon holds message(address to receive asset, amount, etc.), signature and public address and is used to release SOL from solana contract


# Help

## get_ledger_id

```
dfx canister call minter get_ledger_id
```

## get_address
Returns Threshold ECDSA address ("ecdsa_public_key") in 3 formats:
1) compressed public key (size: 33 bytes, generated from icp)
2) uncompressed public key (size: 64 bytes, generated from compressed version via "libsecp256k1" library)

```bash
dfx canister call minter get_address
```

## withdraw
Withdraw burns cksol and provides a coupon

```bash
dfx canister call ledger icrc1_balance_of "(record {
  owner = principal \"$USER_PRINCIPAL\"
})"
dfx canister call ledger icrc1_total_supply

dfx canister call ledger icrc2_approve "(record {
  spender = record { owner = principal \"$(dfx canister id minter)\" };
  amount = 1_000_000_000;
})" --identity $USER_PRINCIPAL_NAME

# provide solana address and amount
dfx canister call minter withdraw "(\"HS6NTv6GBVSLct8dsimRWRvjczJTAgfgDJt8VpR8wtGm\", 100_000)" --identity $USER_PRINCIPAL_NAME
```

Coupon Example:
```rust
{
    /// The recovery ID (y parity) for signature
    recovery_id = opt (0 : nat8);
    /// The hexadecimal representation of the ICP public key in non compressed format.
    icp_public_key_hex = "04de48381e1b54e2463cafdcafc3aaf7d99b1c512a16ac60e6415514d07ab78d6010b31fc919cc196b82ede54859f1d9cd69258f83b5d5bb146a77f326b9a723ab";
      /// The message associated with the coupon.
    /// This message typically contains details about the withdrawal event.
    message = "{"from_icp_address":"svq52-4c5cd-olo3w-r6b37-jizpw-kixdx-uarhl-nolu3-gcikk-nza7z-yae","to_sol_address":"8nZLXraZUARNmU3P8PKbJMS7NYs7aEyw6d1aQx1km3t2","amount":100000,"burn_id":2,"burn_timestamp":1711616761296437000,"icp_burn_block_index":106}";
    /// The signature of the coupon.
    signature_hex = "ac30c685a756feafbe9e34939054fb8e7b0879039f18eb536a06a12483f0f8d25f4e6fc29cf5fbb9742d0e9fff39dbf3bbc3adf3b56477adb614417c4157168a";
    /// The hash of the message associated with the coupon.
    message_hash = "8278c60c27f95ccb2b0956c4b7ed9ef90e1ec67d3d8cf88cec39632d3f0d4bf0";
}
```
No matter who executes the withdrawal process on the Solana side, the asset will be reimbursed to the Solana address provided during the minter canister call.

## get_state

```
dfx canister call minter get_state
```

## get_active_tasks

```
dfx canister call minter get_active_tasks
```

# Known Issues
1) If the user's withdrawal process succeeds in burning ckSOL but fails during signing, the user will not receive a coupon.
   This issue is partially addressed by storing the burn transaction with a burn_id for easy referencing. However, there is
   currently no API method provided for retrying coupon generation.
2) Solana Testnet and Devnet do not retain transactions and transaction signatures for an extended period. This can lead to
   issues in the Solana parser, such as encountering existing signatures without corresponding transaction data. Currently,
   any parsing issue encountered is retried up to 100 times before being dropped. It may be beneficial to implement a mechanism
   with progressively longer retry periods for improved handling of such issues.".