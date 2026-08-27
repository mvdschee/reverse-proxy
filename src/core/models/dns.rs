use crate::core::models::routes::Host;
use crate::string_newtype;
use crate::{Error, Result};
use reqwest::Client;
use serde::Deserialize;

pub trait DnsProvider: Send + Sync + 'static {
	/// Only returns single record
	/// gets all records but internally filters out all records either,
	/// after the call or if it supports it directly on the endpoint call.
	fn get_challenge_record(&self, host: &Host) -> Result<Record>;
	/// Single fuction to set and update
	/// will return the set record (if returned otherwise fake it)
	fn update_challenge_record(&self, host: &Host, value: &str) -> Result<Record>;
}

// --- DNS Record ---
string_newtype!(Record, derive(Deserialize));

// --- DNS Provider credentials ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentail {
	Cloudflare(CloudflareProvider),
}

// --- add here any other providers credentials ---

// --- CloudFlare ---

// config coming from TOML file
#[derive(Debug, Clone, Deserialize)]
pub struct CloudflareProvider {
	pub zone_id: ZoneId,
	pub api_token: ApiToken,
	pub challenge_prefix: String,
}

// struct that hold client
pub struct Cloudflare {
	client: Client,
	config: CloudflareProvider,
}

// Zone ID
string_newtype!(ZoneId, derive(Deserialize));

// API token
string_newtype!(ApiToken, derive(Deserialize));
