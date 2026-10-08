use serde::Deserialize;
use std::ops::Deref;

// --- TASK INTERVAL ---
/// Interval for a task in seconds.
#[derive(Debug, Clone, Deserialize)]
pub struct TaskInterval(u64);

impl Deref for TaskInterval {
	type Target = u64;

	fn deref(&self) -> &u64 {
		&self.0
	}
}

impl From<u64> for TaskInterval {
	fn from(s: u64) -> Self {
		TaskInterval(s)
	}
}
