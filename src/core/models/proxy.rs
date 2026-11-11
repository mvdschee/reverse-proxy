use crate::{
	core::models::routes::{Host, Upstream},
	newtype,
};
use std::collections::HashMap;

pub type ProxyRouteMap = HashMap<Host, ProxyRoute>;

pub struct Sni(pub String);

#[derive(Default)]
pub struct RequestCtx {
	pub tls: bool,
	pub sni: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProxyRoute {
	pub host: Host,
	pub upstream: Upstream,
	pub tls: ProxyTls,
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
	pub http_port: ProxyPort,
	pub https_port: ProxyPort,
	pub input_address: ProxyInputAddress,
}

// --- PROXY TLS ---
newtype!(
   ProxyTls(bool)
);

impl From<bool> for ProxyTls {
	fn from(s: bool) -> Self {
		ProxyTls(s)
	}
}

// --- HTTP(S) PORT ---
newtype!(
   ProxyPort(u16)
);

impl From<u16> for ProxyPort {
	fn from(s: u16) -> Self {
		ProxyPort(s)
	}
}

// --- PROXY INPUT ADDRESS ---
newtype!(
   ProxyInputAddress(String)
);

impl From<String> for ProxyInputAddress {
	fn from(s: String) -> Self {
		ProxyInputAddress(s)
	}
}
