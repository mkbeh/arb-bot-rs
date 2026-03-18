use solana_sdk::pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct PoolUpdate {
    pub changed_pools: Vec<(Pubkey, u64)>,
}

pub struct ComputeService {
    tx: mpsc::Sender<PoolUpdate>,
    rx: mpsc::Receiver<PoolUpdate>,
}

impl ComputeService {
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(1024);
        Self { tx, rx }
    }

    #[must_use]
    pub fn sender(&self) -> mpsc::Sender<PoolUpdate> {
        self.tx.clone()
    }

    pub async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        todo!()
    }
}
