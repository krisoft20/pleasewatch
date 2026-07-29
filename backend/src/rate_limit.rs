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
        let entry = self.attempts.entry(ip).or_default();

        entry.retain(|t| now.duration_since(*t) < self.window);

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
