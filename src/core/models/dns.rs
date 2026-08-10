use serde::Deserialize;
use std::{fmt, ops::Deref};

// --- EMAIL ---
#[derive(Debug, Clone, Deserialize)]
pub struct Record(String);

impl Deref for Record {
	type Target = String;

	fn deref(&self) -> &String {
		&self.0
	}
}

impl fmt::Display for Record {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<String> for Record {
	fn from(s: String) -> Self {
		Record(s)
	}
}
