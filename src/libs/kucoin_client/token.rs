use crate::libs::kucoin_client::{
    Client,
    api::{Api, Spot},
    models::{RestResponse, Token},
};

/// Wrapper struct for basic KuCoin API operations focused on token retrieval.
#[derive(Clone)]
pub struct BaseInfo {
    pub client: Client,
}

impl BaseInfo {
    /// Retrieves a public bullet token from KuCoin.
    pub async fn get_bullet_public(&self) -> anyhow::Result<RestResponse<Token>> {
        self.client
            .post(Api::Spot(Spot::GetBulletPublic), None, None, false)
            .await
    }

    /// Retrieves a private bullet token from KuCoin.
    pub async fn get_bullet_private(&self) -> anyhow::Result<RestResponse<Token>> {
        self.client
            .post(Api::Spot(Spot::GetBulletPrivate), None, None, true)
            .await
    }
}
