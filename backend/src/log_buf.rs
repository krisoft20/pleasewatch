use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use tokio::sync::broadcast;

const MAX_ENTRIES: usize = 10_000;
const BROADCAST_CAP: usize = 512;

#[derive(Clone, Debug, Serialize)]
pub struct LogEntry {
    pub seq: u64,
    pub ts: String,
    pub level: String,
    pub tag: String,
    pub msg: String,
}

static RING: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();
static TX: OnceLock<broadcast::Sender<LogEntry>> = OnceLock::new();
static SEQ: OnceLock<Mutex<u64>> = OnceLock::new();

fn ring() -> &'static Mutex<VecDeque<LogEntry>> {
    RING.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_ENTRIES)))
}

fn tx() -> &'static broadcast::Sender<LogEntry> {
    TX.get_or_init(|| broadcast::channel(BROADCAST_CAP).0)
}

fn next_seq() -> u64 {
    let m = SEQ.get_or_init(|| Mutex::new(0));
    let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
    *g += 1;
    *g
}

pub fn push(level: &str, tag: &str, msg: &str) {
    let entry = LogEntry {
        seq: next_seq(),
        ts: chrono::Utc::now().format("%H:%M:%S").to_string(),
        level: level.to_string(),
        tag: tag.to_string(),
        msg: msg.to_string(),
    };
    {
        let mut r = ring().lock().unwrap_or_else(|e| e.into_inner());
        if r.len() >= MAX_ENTRIES {
            r.pop_front();
        }
        r.push_back(entry.clone());
    }
    let _ = tx().send(entry);
}

pub fn snapshot() -> Vec<LogEntry> {
    ring()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

pub fn subscribe() -> broadcast::Receiver<LogEntry> {
    tx().subscribe()
}

pub fn split_tag(m: &str) -> (String, String) {
    if m.starts_with('[') {
        if let Some(end) = m.find(']') {
            let tag = m[1..end].to_string();
            let msg = m[end + 1..].trim_start().to_string();
            return (tag, msg);
        }
    }
    (String::from("?"), m.to_string())
}

#[macro_export]
macro_rules! pi {
    ($($arg:tt)*) => {{
        let m = std::format!($($arg)*);
        std::println!("{}", m);
        let (tag, msg) = $crate::log_buf::split_tag(&m);
        $crate::log_buf::push("info", &tag, &msg);
    }};
}

#[macro_export]
macro_rules! pe {
    ($($arg:tt)*) => {{
        let m = std::format!($($arg)*);
        std::eprintln!("{}", m);
        let (tag, msg) = $crate::log_buf::split_tag(&m);
        $crate::log_buf::push("error", &tag, &msg);
    }};
}
