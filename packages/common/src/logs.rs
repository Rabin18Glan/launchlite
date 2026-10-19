use crate::models::{LogLine, current_timestamp};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const MAX_LOG_LINES: usize = 500;

#[derive(Debug, Clone)]
pub struct LogBuffer {
    capacity: usize,
    lines: Arc<Mutex<VecDeque<LogLine>>>,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            lines: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
        }
    }

    pub fn push(&self, stream: &str, message: String) {
        let mut lines = self.lines.lock().unwrap();
        if lines.len() >= self.capacity {
            lines.pop_front();
        }
        lines.push_back(LogLine {
            timestamp: current_timestamp(),
            stream: stream.to_string(),
            message,
        });
    }

    pub fn get_lines(&self) -> Vec<LogLine> {
        let lines = self.lines.lock().unwrap();
        lines.iter().cloned().collect()
    }
}
