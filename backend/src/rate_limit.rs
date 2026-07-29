use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    attempts: HashMap<IpAddr, Vec<Instant>>,
    max_attempts: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_attempts: usize, window_secs: u64) -> Self {
        Self {
            attempts: HashMap::new(),
            max_attempts,
            window: Duration::from_secs(window_secs),
        }
    }

    pub fn check(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let window = self.window;
        self.attempts.retain(|_, attempts| {
            attempts.retain(|t| now.duration_since(*t) < window);
            !attempts.is_empty()
        });

        let entry = self.attempts.entry(ip).or_default();

        if entry.len() >= self.max_attempts {
            return false;
        }

        entry.push(now);
        true
    }

    pub fn reset(&mut self, ip: IpAddr) {
        self.attempts.remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clients_have_separate_limits() {
        let mut limiter = RateLimiter::new(2, 60);
        let first = "203.0.113.8".parse().unwrap();
        let second = "198.51.100.4".parse().unwrap();

        assert!(limiter.check(first));
        assert!(limiter.check(first));
        assert!(!limiter.check(first));
        assert!(limiter.check(second));

        limiter.reset(first);
        assert!(limiter.check(first));
    }

    #[test]
    fn expired_clients_are_removed() {
        let mut limiter = RateLimiter::new(1, 0);
        let first = "203.0.113.8".parse().unwrap();
        let second = "198.51.100.4".parse().unwrap();

        assert!(limiter.check(first));
        assert!(limiter.check(second));
        assert!(!limiter.attempts.contains_key(&first));
    }
}
