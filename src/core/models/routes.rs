use crate::{
	core::models::{certs::CertificateType, dns::ProviderCredentail},
	newtype,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Route {
	pub host: Host,
	pub upstream: Upstream,
	#[serde(default)]
	pub cert_type: CertificateType,
	pub dns_provider: Option<ProviderCredentail>,
}

// --- HOST ---
newtype!(
   Host(String),
   derive(Deserialize, Hash, Eq)
);

impl From<String> for Host {
	fn from(s: String) -> Self {
		Host(s)
	}
}

impl std::borrow::Borrow<str> for Host {
	fn borrow(&self) -> &str {
		&self.0
	}
}

// --- UPSTREAM ---
newtype!(
   Upstream(String),
   derive(Deserialize)
);

impl From<String> for Upstream {
	fn from(s: String) -> Self {
		Upstream(s)
	}
}
