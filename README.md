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
        amount: 10_000_000,
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
  '("fff722f41eb1cae151458c2d9acf16695984d1376a6a0a6ab56a385204f889370aec600906f278c5d9522d1df16ab50940827f96d7f62f61cd2ba33b28f2b7df", "{\"address\":\"9gVndQ5SdugdFfGzyuKmePLRJZkCreKZ2iUTEg4agR5g\",\"amount\":10000000}", "0269b3e4f4295275d99217c8d4f31a872d7af55c671fde7b0ed293650f9d1a4115")'
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
  '("fff722f41eb1cae151458c2d9acf16695984d1376a6a0a6ab56a385204f889370aec600906f278c5d9522d1df16ab50940827f96d7f62f61cd2ba33b28f2b7df", "{\"address\":\"9gVndQ5SdugdFfGzyuKmePLRJZkCreKZ2iUTEg4agR5g\",\"amount\":10000000}", "0269b3e4f4295275d99217c8d4f31a872d7af55c671fde7b0ed293650f9d1a4115")'

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