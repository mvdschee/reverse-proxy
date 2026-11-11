use crate::newtype;
use serde::Deserialize;

// --- TASK INTERVAL ---
newtype!(
   /// Interval for a task in seconds.
   TaskInterval(u64),
   derive(Deserialize)
);

impl From<u64> for TaskInterval {
	fn from(s: u64) -> Self {
		TaskInterval(s)
	}
}
