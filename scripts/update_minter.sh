#!/bin/bash

# deploy minter canister
dfx deploy minter --upgrade-unchanged --argument "
  (variant {
    Upgrade = record {
    }
  })
" --yes
