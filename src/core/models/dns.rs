use crate::{Result, config::ACME_CHALLENGE_PREFIX, core::models::routes::Host, newtype};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

#[async_trait]
pub trait DnsProvider {
	/// Single fuction to set and update
	/// will return the set record (if returned otherwise fake it)
	async fn upsert_challenge_record(&self, dns_value: String) -> Result<Record>;
}

pub fn default_challenge_prefix() -> ChallengePrefix {
	ChallengePrefix::from(ACME_CHALLENGE_PREFIX.to_string())
}

// --- DNS Record ---
pub struct Record {
	#[expect(dead_code, reason = "used later to update/delete the record")]
	pub provider_id: RecordId,
	pub name: String,
	pub value: String,
}

// --- Challenge Prefix (ex: _acme-challenge.) ---
newtype!(
   ChallengePrefix(String),
   derive(Deserialize)
);

impl From<String> for ChallengePrefix {
	fn from(s: String) -> Self {
		ChallengePrefix(s)
	}
}

newtype!(
   /// API token can be for any provider.
   /// we can safely asume that this will always be a string
   ApiToken(String),
   derive(Deserialize)
);

impl From<String> for ApiToken {
	fn from(s: String) -> Self {
		ApiToken(s)
	}
}

newtype!(
   /// DNS Record ID, meant for any provider
   /// the dns provider required to update a record
   RecordId(String),
   derive(Deserialize)
);

impl From<String> for RecordId {
	fn from(s: String) -> Self {
		RecordId(s)
	}
}

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

newtype!(
   /// Zone ID
   CFZoneId(String),
   derive(Deserialize)
);

impl From<String> for CFZoneId {
	fn from(s: String) -> Self {
		CFZoneId(s)
	}
}
