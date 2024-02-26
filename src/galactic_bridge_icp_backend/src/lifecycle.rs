//! Module dealing with the lifecycle methods of the ckETH Minter.
use crate::lifecycle::init::InitArg;
use crate::lifecycle::upgrade::UpgradeArg;
use candid::{CandidType, Deserialize};
use std::fmt::{Display, Formatter};

pub mod init;
pub mod upgrade;
pub use upgrade::post_upgrade;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum MinterArg {
    InitArg(InitArg),
    UpgradeArg(UpgradeArg),
}

// TODO: ENCODE / DECODE minicbor
#[derive(CandidType, Clone, Copy, Default, Deserialize, Debug, Eq, PartialEq, Hash)]
pub enum SolanaNetwork {
    Mainnet,
    #[default]
    Testnet,
}

// TODO: solana doesn't have a chain id, so this is not used
// impl EthereumNetwork {
//     pub fn chain_id(&self) -> u64 {
//         match self {
//             EthereumNetwork::Mainnet => 1,
//             EthereumNetwork::Sepolia => 11155111,
//         }
//     }
// }

impl Display for SolanaNetwork {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SolanaNetwork::Mainnet => write!(f, "Solana Mainnet"),
            SolanaNetwork::Sepolia => write!(f, "Solana Sepolia"),
        }
    }
}
