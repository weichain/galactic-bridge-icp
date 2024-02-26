pub(crate) const MAINNET_PROVIDERS: [RpcNodeProvider; 1] =
    [RpcNodeProvider::SolanaMainnet(SolanaMainnetProvider::Free)];

pub(crate) const TESTNET_PROVIDERS: [RpcNodeProvider; 1] =
    [RpcNodeProvider::SolanaTestnet(SolanaTestnetProvider::Free)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum RpcNodeProvider {
    SolanaMainnet(SolanaMainnetProvider),
    SolanaTestnet(SolanaTestnetProvider),
}

impl RpcNodeProvider {
    pub(crate) fn url(&self) -> &str {
        match self {
            Self::SolanaMainnet(provider) => provider.solana_mainnet_endpoint_url(),
            Self::SolanaTestnet(provider) => provider.solana_testnet_endpoint_url(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum SolanaMainnetProvider {
    // https://www.ankr.com/rpc/
    Free,
}

impl SolanaMainnetProvider {
    fn solana_mainnet_endpoint_url(&self) -> &str {
        match self {
            SolanaMainnetProvider::Free => "https://api.mainnet-beta.solana.com",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum SolanaTestnetProvider {
    // https://api.testnet.solana.com
    Free,
}

impl SolanaTestnetProvider {
    fn solana_testnet_endpoint_url(&self) -> &str {
        match self {
            SolanaTestnetProvider::Free => "https://api.testnet.solana.com",
        }
    }
}
