use crate::core::models::{
	certs::CertDir,
	filesystem::SafePath,
	routes::{Host, Upstream},
};
use http::{Response, StatusCode, header};
use std::{collections::HashMap, fmt, ops::Deref, sync::Arc};

pub type ProxyRouteMap = Arc<HashMap<Host, ProxyRoute>>;

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
#[derive(Debug, Clone)]
pub struct ProxyTls(bool);

impl Deref for ProxyTls {
	type Target = bool;

	fn deref(&self) -> &bool {
		&self.0
	}
}

impl From<bool> for ProxyTls {
	fn from(s: bool) -> Self {
		ProxyTls(s)
	}
}

// --- HTTP(S) PORT ---
#[derive(Debug, Clone)]
pub struct ProxyPort(u16);

impl Deref for ProxyPort {
	type Target = u16;

	fn deref(&self) -> &u16 {
		&self.0
	}
}

impl From<u16> for ProxyPort {
	fn from(s: u16) -> Self {
		ProxyPort(s)
	}
}

// --- PROXY INPUT ADDRESS ---
#[derive(Debug, Clone)]
pub struct ProxyInputAddress(String);

impl Deref for ProxyInputAddress {
	type Target = String;

	fn deref(&self) -> &String {
		&self.0
	}
}

impl fmt::Display for ProxyInputAddress {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<String> for ProxyInputAddress {
	fn from(s: String) -> Self {
		ProxyInputAddress(s)
	}
}
