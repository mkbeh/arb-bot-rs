use std::time::{SystemTime, UNIX_EPOCH};

pub fn build_query(params: &Vec<(String, String)>) -> String {
    let mut query = String::new();
    for (k, v) in params {
        query.push_str(&format!("{}={}&", k, v));
    }
    query.pop();
    query
}

pub fn get_timestamp(start: SystemTime) -> anyhow::Result<u64> {
    let since_epoch = start.duration_since(UNIX_EPOCH)?;
    Ok(since_epoch.as_secs() * 1000 + u64::from(since_epoch.subsec_nanos()) / 1_000_000)
}
