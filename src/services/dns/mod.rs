use crate::core::models::dns::{CloudflareProvider, Record};
use crate::core::models::routes::Host;
use crate::{Error, Result};

mod cloudflare;

trait DnsProvider: Send + Sync + 'static {
	/// Only returns single record
	/// gets all records but internally filters out all records either,
	/// after the call or if it supports it directly on the endpoint call.
	fn get_challenge_record(&self, host: Host) -> Result<Record>;
	/// Single fuction to set and update
	/// will return the set record (if returned otherwise fake it)
	fn update_challenge_record(&self, host: Host) -> Result<Record>;
}
