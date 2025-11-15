use std::sync::LazyLock;

use tokio::sync::Mutex;

use crate::libs::misc;

/// Global request weight limiter.
pub static REQUEST_WEIGHT: LazyLock<Mutex<RequestWeight>> =
    LazyLock::new(|| Mutex::new(RequestWeight::default()));

/// Manages request weight limits with time-based resets.
pub struct RequestWeight {
    timestamp: u64,
    weight: usize,
    weight_limit: usize,
    weight_reset_secs: u64,
}

impl Default for RequestWeight {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestWeight {
    pub fn new() -> Self {
        Self {
            timestamp: misc::time::get_current_timestamp().as_secs(),
            weight: 0,
            weight_limit: 0,
            weight_reset_secs: 60,
        }
    }

    /// Sets the maximum allowed weight.
    pub fn set_weight_limit(&mut self, weight_limit: usize) {
        self.weight_limit = weight_limit;
    }

    /// Attempts to add weight; returns true if successful (under limit after reset check)
    pub fn add(&mut self, weight: usize) -> bool {
        let current_ts = misc::time::get_current_timestamp().as_secs();
        if current_ts - self.timestamp > self.weight_reset_secs {
            self.weight = 0;
            self.timestamp = current_ts;
        }

        if self.weight + weight > self.weight_limit {
            return false;
        };

        self.weight += weight;
        true
    }

    /// Subtracts weight if possible (no underflow).
    pub fn sub(&mut self, weight: usize) {
        if weight < self.weight {
            self.weight -= weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use tokio::{sync::Mutex, task::JoinSet};

    use crate::services::weight::RequestWeight;

    #[test]
    fn test_request_weight_add() -> anyhow::Result<()> {
        let mut request_weight = RequestWeight::new();
        request_weight.set_weight_limit(10);

        let result = request_weight.add(5);
        assert!(result);
        assert_eq!(request_weight.weight, 5);

        let result = request_weight.add(10);
        assert!(!result);
        assert_eq!(request_weight.weight, 5);

        Ok(())
    }

    #[tokio::test]
    async fn test_request_weight_add_async() -> anyhow::Result<()> {
        static RW: LazyLock<Mutex<RequestWeight>> =
            LazyLock::new(|| Mutex::new(RequestWeight::default()));

        {
            let mut guard = RW.lock().await;
            guard.set_weight_limit(10);
        }

        let mut set = JoinSet::new();
        for _ in 0..10 {
            set.spawn(async move {
                let mut guard = RW.lock().await;
                let _ = guard.add(10);
                assert_eq!(guard.weight, 10);
            });
        }

        for _ in 0..10 {
            set.join_next().await;
        }

        Ok(())
    }

    #[test]
    fn test_request_weight_sub() -> anyhow::Result<()> {
        let mut request_weight = RequestWeight::new();
        request_weight.set_weight_limit(10);

        request_weight.sub(5);
        assert_eq!(request_weight.weight, 0);

        let result = request_weight.add(5);
        assert!(result);
        assert_eq!(request_weight.weight, 5);

        request_weight.sub(1);
        assert_eq!(request_weight.weight, 4);

        Ok(())
    }
}
