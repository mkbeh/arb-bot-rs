use std::sync::LazyLock;

use tokio::sync::Mutex;

use crate::libs::misc;

pub static REQUEST_WEIGHT: LazyLock<Mutex<RequestWeight>> =
    LazyLock::new(|| Mutex::new(RequestWeight::default()));

pub struct RequestWeight {
    timestamp: u64,
    pub weight: usize,
    pub weight_limit: usize,
    pub weight_reset_secs: u64,
}

impl Default for RequestWeight {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestWeight {
    pub fn new() -> Self {
        Self {
            timestamp: misc::time::get_current_timestamp(),
            weight: 0,
            weight_limit: 0,
            weight_reset_secs: 60,
        }
    }

    pub fn set_weight_limit(&mut self, weight_limit: usize) {
        self.weight_limit = weight_limit;
    }

    pub fn add(&mut self, weight: usize) -> bool {
        if (misc::time::get_current_timestamp() - self.timestamp) > self.weight_reset_secs {
            self.weight = 0
        }

        if self.weight + weight > self.weight_limit {
            return false;
        };

        self.weight += weight;
        true
    }

    pub fn sub(&mut self, weight: usize) {
        if weight < self.weight {
            self.weight -= weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::services::binance::RequestWeight;

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
