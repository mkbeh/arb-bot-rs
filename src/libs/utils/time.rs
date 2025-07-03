#[cfg(not(all(
    target_arch = "wasm32",
    not(any(target_os = "emscripten", target_os = "wasi"))
)))]
#[must_use]
pub fn get_current_timestamp() -> u64 {
    let start = std::time::SystemTime::now();
    start
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}
