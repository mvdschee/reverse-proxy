use crate::{
	Error, Result, config::ACME_CHALLENGE_PREFIX, core::models::routes::Host, string_newtype,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

#[async_trait]
pub trait DnsProvider {
	/// Single fuction to set and update
	/// will return the set record (if returned otherwise fake it)
	async fn upsert_challenge_record(&self, dns_value: String) -> Result<Record>;
}

pub fn default_challenge_prefix() -> String {
	ACME_CHALLENGE_PREFIX.to_string()
}

// --- DNS Record ---
pub struct Record {
	pub provider_id: RecordId,
	pub name: String,
	pub value: String,
}

// --- Challenge Prefix (ex: _acme-challenge.) ---
string_newtype!(ChallengePrefix, derive(Deserialize));

// API token can be for any provider.
// we can safely asume that this will always be a string
string_newtype!(ApiToken, derive(Deserialize));

// DNS Record ID, meant for any provider
// the dns provider required to update a record
string_newtype!(RecordId, derive(Deserialize));

// --- DNS Provider credentials ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentail {
	Cloudflare(CloudflareProvider),
}

// --- add here any other providers credentials or needed structs ---

// --- CloudFlare ---
// ASSIGNED TYPE PREFIX: CF

// config struct, define here your values that is needed,
// for the runtime struct
#[derive(Debug, Clone, Deserialize)]
pub struct CloudflareProvider {
	pub api_token: ApiToken,
	pub zone_id: CFZoneId,
}

// runtime struct that will actually do the DNS updating
pub struct Cloudflare {
	pub client: Client,
	pub host: Host,
	pub config: CloudflareProvider,
	pub challenge_prefix: ChallengePrefix,
}

// Zone ID
string_newtype!(CFZoneId, derive(Deserialize));
