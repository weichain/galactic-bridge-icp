# galactic_bridge_icp

## Deployment
Deploys ledger and minter

```bash
dfx start --background

./scripts/export.sh
./scripts/deploy_all.sh
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

## get_address
Returns Threshold ECDSA address ("ecdsa_public_key") in 3 formats:
1) compressed public key (size: 33 bytes, generated from icp)
2) uncompressed public key (size: 64 bytes, generated from compressed version via "libsecp256k1" library)
3) ethereum address (size: 20 bytes)

```bash
dfx canister call minter get_address
```

## sign message
Signs a predefined message with "sign_with_ecdsa":
```
    let coupon = Coupon {
        address: "9gVndQ5SdugdFfGzyuKmePLRJZkCreKZ2iUTEg4agR5g".to_string(),
        amount: 10_000_000_000,
    };
```

Returns:
1) coupon in json format
2) hex of coupon
3) signature

```bash
dfx canister call minter sign
```

## verify
Input:
1) signature
2) message
3) compressed key pair (via "k256" library)

Return:
true/false

Example:
```bash
dfx canister call minter verify \
  '("11423bfa8d4899d97318a846f799520c49a3c36e4ef499f35d5b9a480edbc6393760740a6f6194550439b29d815357e0fb07574f4157bc82ac3c9c9f7702b8a8", "{\"address\":\"9gVndQ5SdugdFfGzyuKmePLRJZkCreKZ2iUTEg4agR5g\",\"amount\":10000000000}", "020ab24d427257b0a726c5c8d3cad5fed2cfcd44a4ffa93f4ce239af2a4bce32e2")'
```

## y_parity / recovery id
Input:
1) signature
2) message
3) compressed public key (via "k256" library)

Return: 
Recovery id -> 0/1

```bash
dfx canister call minter y_parity \
  '("11423bfa8d4899d97318a846f799520c49a3c36e4ef499f35d5b9a480edbc6393760740a6f6194550439b29d815357e0fb07574f4157bc82ac3c9c9f7702b8a8", "{\"address\":\"9gVndQ5SdugdFfGzyuKmePLRJZkCreKZ2iUTEg4agR5g\",\"amount\":10000000000}", "020ab24d427257b0a726c5c8d3cad5fed2cfcd44a4ffa93f4ce239af2a4bce32e2")'

```

## get_ledger_id

```
dfx canister call minter get_ledger_id
```

## mint
Mint method manually generates ckSol asset.
This functionality will become part of solana scrapper.

```bash
dfx canister call minter mint "(principal \"$USER_PRINCIPAL\", 1_000)"

dfx canister call ledger icrc1_balance_of "(record { owner = principal \"$USER_PRINCIPAL\" })"
```

## burn
Mint method manually removes ckSol asset.
This functionality will become part of the withdraw flow.

```bash
dfx canister call ledger icrc2_approve "(record {
    spender = record { owner = principal \"$(dfx canister id minter)\" };
    amount = 1_000_000;
  })" --identity $USER_PRINCIPAL_NAME

dfx canister call minter burn "(1_000)" --identity $USER_PRINCIPAL_NAME

dfx canister call ledger icrc1_balance_of "(record { owner = principal \"$USER_PRINCIPAL\" })"

dfx canister call ledger icrc1_total_supply
```