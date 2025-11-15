use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<sha2::Sha256>;
pub fn sign(plain: &str, key: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(plain.as_bytes());
    general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

pub fn get_timestamp(start: SystemTime) -> anyhow::Result<u64> {
    let since_epoch = start.duration_since(UNIX_EPOCH)?;
    Ok(since_epoch.as_secs() * 1000 + u64::from(since_epoch.subsec_nanos()) / 1_000_000)
}
