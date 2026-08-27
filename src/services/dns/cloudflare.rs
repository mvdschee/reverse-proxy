use serde::de::value;

use crate::core::models::dns::{CloudflareProvider, DnsProvider, Record};
use crate::core::models::routes::Host;
use crate::{Error, Result};

impl DnsProvider for CloudflareProvider {
	fn get_challenge_record(&self, host: &Host) -> Result<Record> {
		Err(Error::Dns("not implemented!".to_string()))
	}
	fn update_challenge_record(&self, host: &Host, value: &str) -> Result<Record> {
		Err(Error::Dns("not implemented!".to_string()))
	}
}
