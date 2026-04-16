/// Parses the token amount from a raw SPL token account data buffer.
///
/// SPL token account layout:
/// - `[0..32]`  — mint pubkey
/// - `[32..64]` — owner pubkey
/// - `[64..72]` — amount (u64, little-endian)
///
/// This works for both `spl-token` and `spl-token-2022` accounts
/// since the base layout is identical.
///
/// # Errors
/// Returns an error if the data buffer is too short (less than 72 bytes).
pub fn parse_vault_amount(data: &[u8]) -> anyhow::Result<u64> {
    let bytes = data
        .get(64..72)
        .ok_or_else(|| anyhow::anyhow!("vault account data too short"))?;
    Ok(u64::from_le_bytes(bytes.try_into()?))
}
