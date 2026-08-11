use crate::string_newtype;
use serde::Deserialize;

// --- DNS Record ---
string_newtype!(Record, derive(Deserialize));

// --- DNS Provider credentials ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentails {
	Cloudflare(CloudflareProvider),
}

// --- add here any other providers credentials ---

// --- CloudFlare ---
#[derive(Debug, Clone, Deserialize)]
pub struct CloudflareProvider {
	pub zone_id: ZoneId,
	pub api_token: ApiToken,
}

// Zone ID
string_newtype!(ZoneId, derive(Deserialize));

// API token
string_newtype!(ApiToken, derive(Deserialize));
