use serde_bytes::ByteBuf;
use std::time::Duration;

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
pub const DERIVATION_PATH: Vec<ByteBuf> = vec![];

pub const GET_LATEST_SOLANA_SIGNATURE: Duration = Duration::from_secs(1 * 60);
pub const SCRAPPING_SOLANA_SIGNATURE_RANGES: Duration = Duration::from_secs(3 * 60);
pub const SCRAPPING_SOLANA_SIGNATURES: Duration = Duration::from_secs(3 * 60);
pub const MINT_GSOL: Duration = Duration::from_secs(3 * 60);

pub const SOLANA_SIGNATURE_RANGES_RETRY_LIMIT: u8 = 100;
pub const SOLANA_SIGNATURE_RETRY_LIMIT: u8 = 100;
pub const MINT_GSOL_RETRY_LIMIT: u8 = 100;
