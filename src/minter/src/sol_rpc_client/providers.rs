// TODO: attach more providers
pub(crate) const MAINNET_PROVIDERS: [RpcNodeProvider; 1] =
    [RpcNodeProvider::Mainnet(SolanaMainnetProvider::PublicNode)];

pub(crate) const TESTNET_PROVIDERS: [RpcNodeProvider; 1] =
    [RpcNodeProvider::Testnet(SolanaTestnetProvider::PublicNode)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum RpcNodeProvider {
    Mainnet(SolanaMainnetProvider),
    Testnet(SolanaTestnetProvider),
}

impl RpcNodeProvider {
    pub(crate) fn url(&self) -> &str {
        match self {
            Self::Mainnet(provider) => provider.endpoint_url(),
            Self::Testnet(provider) => provider.endpoint_url(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum SolanaMainnetProvider {
    PublicNode,
}

impl SolanaMainnetProvider {
    fn endpoint_url(&self) -> &str {
        match self {
            SolanaMainnetProvider::PublicNode => "https://api.mainnet-beta.solana.com",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) enum SolanaTestnetProvider {
    PublicNode,
}

impl SolanaTestnetProvider {
    fn endpoint_url(&self) -> &str {
        match self {
            SolanaTestnetProvider::PublicNode => "https://api.devnet.solana.com",
        }
    }
}
