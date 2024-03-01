// The derivation path to use for ECDSA secp256k1.
// First component: Hardened derivation for purpose (44')
// vec![0x80, 44],
// Second component: Hardened derivation for coin type (60')
// vec![0x80, 60],
// Third component: Hardened derivation for account (0')
// vec![0x80, 0],
// Fourth component: Non-hardened derivation for external/internal flag (0 for external, 1 for internal)
// vec![0],
// Fifth component: Non-hardened derivation for index (0)
// vec![1],
pub static DERIVATION_PATH: Vec<Vec<u8>> = vec![];
