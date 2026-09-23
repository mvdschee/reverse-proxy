use crate::{
	Error, Result,
	core::models::{
		dns::{Cloudflare, DnsProvider, Record, RecordId, default_challenge_prefix},
		routes::Host,
	},
	info,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use http::HeaderValue;
use serde::Deserialize;
use std::collections::HashMap;

const API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// CloudflareResponse can be used for both Vec<DnsRecord> or DnsRecord
#[derive(Debug, Deserialize)]
pub struct CloudflareResponse<T> {
	#[serde(default)]
	pub errors: Vec<CloudflareMessage>,
	#[serde(default)]
	pub messages: Vec<CloudflareMessage>,
	pub success: bool,
	pub result: T,
}

// some fields are skipped that provides nothing
#[derive(Debug, Deserialize)]
pub struct CloudflareMessage {
	pub code: u32,
	pub message: String,
	pub documentation_url: Option<String>,
}

// some fields are skipped that provides nothing
#[derive(Debug, Deserialize)]
pub struct DnsRecord {
	pub id: String,
	pub name: String,
	#[serde(rename = "type")]
	pub record_type: String,
	pub content: String,
	pub ttl: u32,
	pub proxied: bool,
	pub comment: Option<String>,
	pub created_on: DateTime<Utc>,
	pub modified_on: DateTime<Utc>,
}

#[async_trait]
impl DnsProvider for Cloudflare {
	async fn upsert_challenge_record(&self, dns_value: String) -> Result<Record> {
		// return on error so we are not
		let record_id = self.get_record_id().await?;

		let Some(record_id) = record_id else {
			// add record
			return self.create_record(dns_value).await;
		};

		// update record
		return self.update_record(record_id, dns_value).await;
	}
}

impl Cloudflare {
	async fn get_record_id(self: &Cloudflare) -> Result<Option<RecordId>> {
		info!("get records for: {}", &self.host);

		let mut headers = reqwest::header::HeaderMap::new();
		headers.insert(
			"Authorization",
			HeaderValue::from_str(&format!("Bearer {}", self.config.api_token))
				.map_err(|e| Error::Dns(e.to_string()))?,
		);
		headers.insert(
			"Content-Type",
			HeaderValue::from_str("application/json").map_err(|e| Error::Dns(e.to_string()))?,
		);

		let url = format!(
			"{}/zones/{}/dns_records?type=TXT&name={}{}",
			API_BASE, self.config.zone_id, self.challenge_prefix, self.host
		);

		let response = match self.client.get(&url).headers(headers).send().await {
			Ok(response) => response,
			Err(e) => return Err(Error::Dns(format!("Request {}", e))),
		};

		let status = response.status();
		if !status.is_success() {
			return Err(Error::Dns(format!("Status {}", status)));
		}

		let response = match response.text().await {
			Ok(response) => response,
			Err(e) => return Err(Error::Dns(format!("Parse {}", e))),
		};

		let result: CloudflareResponse<Vec<DnsRecord>> =
			serde_json::from_str(&response).map_err(|e| Error::Dns(format!("serde_json {}", e)))?;

		// get first item from results and than get record_id
		let record_id = result.result.first().map(|record| RecordId::from(record.id.clone()));

		Ok(record_id)
	}

	async fn create_record(&self, dns_value: String) -> Result<Record> {
		info!("create record for: {}", &self.host);

		let mut headers = reqwest::header::HeaderMap::new();
		headers.insert(
			"Authorization",
			HeaderValue::from_str(&format!("Bearer {}", self.config.api_token))
				.map_err(|e| Error::Dns(e.to_string()))?,
		);
		headers.insert(
			"Content-Type",
			HeaderValue::from_str("application/json").map_err(|e| Error::Dns(e.to_string()))?,
		);

		let challenge_prefix = format!("{}{}", self.challenge_prefix, self.host);

		let json = &serde_json::json!({
			"type": "TXT",
			"name": challenge_prefix,
			"content": dns_value,
			"ttl": 1,
			"proxied": false,
			"comment": "automated with reverse-proxy. don't touch :pray:"
		});

		let url = format!("{}/zones/{}/dns_records", API_BASE, self.config.zone_id);

		let response = match self.client.post(&url).headers(headers).json(json).send().await {
			Ok(response) => response,
			Err(e) => return Err(Error::Dns(format!("Request {}", e))),
		};

		let response = match response.text().await {
			Ok(response) => response,
			Err(e) => return Err(Error::Dns(format!("Parse {}", e))),
		};

		let result: CloudflareResponse<DnsRecord> =
			serde_json::from_str(&response).map_err(|e| Error::Dns(format!("serde_json {}", e)))?;

		let record = Record {
			provider_id: RecordId::from(result.result.id),
			name: result.result.name,
			value: result.result.content,
		};

		Ok(record)
	}

	async fn update_record(&self, record_id: RecordId, dns_value: String) -> Result<Record> {
		info!("update record for: {}", &self.host);

		let mut headers = reqwest::header::HeaderMap::new();
		headers.insert(
			"Authorization",
			HeaderValue::from_str(&format!("Bearer {}", self.config.api_token))
				.map_err(|e| Error::Dns(e.to_string()))?,
		);
		headers.insert(
			"Content-Type",
			HeaderValue::from_str("application/json").map_err(|e| Error::Dns(e.to_string()))?,
		);

		let challenge_prefix = format!("{}{}", self.challenge_prefix, self.host);

		let json = &serde_json::json!({
			"type": "TXT",
			"name": challenge_prefix,
			"content": dns_value,
			"ttl": 1,
			"proxied": false,
			"comment": "automated with reverse-proxy. don't touch :pray:"
		});

		let url = format!("{}/zones/{}/dns_records/{}", API_BASE, self.config.zone_id, record_id);

		let response = match self.client.patch(&url).headers(headers).json(json).send().await {
			Ok(response) => response,
			Err(e) => return Err(Error::Dns(format!("Request {}", e))),
		};

		let status = response.status();
		if !status.is_success() {
			return Err(Error::Dns(format!("Status {}", status)));
		}

		let response = match response.text().await {
			Ok(response) => response,
			Err(e) => return Err(Error::Dns(format!("Parse {}", e))),
		};

		let result: CloudflareResponse<DnsRecord> =
			serde_json::from_str(&response).map_err(|e| Error::Dns(format!("serde_json {}", e)))?;

		let record = Record {
			provider_id: RecordId::from(result.result.id),
			name: result.result.name,
			value: result.result.content,
		};

		Ok(record)
	}
}
