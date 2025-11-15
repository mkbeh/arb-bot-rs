use crate::libs::kucoin_api::{
    Client,
    api::{Api, Spot},
    models::{RestResponse, Token},
};

#[derive(Clone)]
pub struct BaseInfo {
    pub client: Client,
}

impl BaseInfo {
    pub async fn get_bullet_public(&self) -> anyhow::Result<RestResponse<Token>> {
        self.client
            .post(Api::Spot(Spot::GetBulletPublic), None, None, false)
            .await
    }

    pub async fn get_bullet_private(&self) -> anyhow::Result<RestResponse<Token>> {
        self.client
            .post(Api::Spot(Spot::GetBulletPrivate), None, None, true)
            .await
    }
}
