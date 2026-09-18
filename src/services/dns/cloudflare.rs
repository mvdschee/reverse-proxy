use serde::de::value;

use crate::{
	Error, Result,
	core::models::{
		dns::{CloudflareProvider, DnsProvider, Record},
		routes::Host,
	},
};

impl DnsProvider for CloudflareProvider {
	fn get_challenge_record(&self, host: &Host) -> Result<Record> {
		Err(Error::Dns("not implemented!".to_string()))
	}
	fn update_challenge_record(&self, host: &Host, value: &str) -> Result<Record> {
		Err(Error::Dns("not implemented!".to_string()))
	}
}
