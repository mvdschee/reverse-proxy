use crate::{
	core::models::{certs::CertificateType, dns::ProviderCredentail},
	string_newtype,
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
string_newtype!(Host, derive(Deserialize, Hash, Eq, PartialEq));

impl std::borrow::Borrow<str> for Host {
	fn borrow(&self) -> &str {
		&self.0
	}
}

// --- UPSTREAM ---
string_newtype!(Upstream, derive(Deserialize));
