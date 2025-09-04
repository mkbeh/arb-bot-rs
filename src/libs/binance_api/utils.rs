use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn generate_signature(secret: &str, query: Option<&str>) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("invalid length of secret key");

    if let Some(q) = query {
        mac.update(q.as_bytes());
    }

    hex::encode(mac.finalize().into_bytes())
}

pub fn get_timestamp(start: SystemTime) -> anyhow::Result<u64> {
    let since_epoch = start.duration_since(UNIX_EPOCH)?;
    Ok(since_epoch.as_secs() * 1000 + u64::from(since_epoch.subsec_nanos()) / 1_000_000)
}
