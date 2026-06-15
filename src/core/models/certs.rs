use crate::core::models::{filesystem::SafePath, routes::Host};
use arc_swap::ArcSwap;
use pingora::tls::{
	pkey::{PKey, Private},
	x509::X509,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CertificateType {
	SelfSigned,
	#[default]
	Acme,
	None,
}

#[derive(Debug, Clone)]
pub struct CertificateConfig {
	pub host: Host,
	pub cert_dir: CertDir,
	pub cert_type: CertificateType,
}

pub struct TlsMaterial {
	pub cert: X509,
	pub key: PKey<Private>,
}

pub type KeyPath = SafePath;
pub type CertPath = SafePath;

pub type TlsStore = Arc<ArcSwap<HashMap<Host, TlsMaterial>>>;

// --- EMAIL ---
#[derive(Debug, Clone, Deserialize)]
pub struct Email(String);

impl Deref for Email {
	type Target = String;

	fn deref(&self) -> &String {
		&self.0
	}
}

impl fmt::Display for Email {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<String> for Email {
	fn from(s: String) -> Self {
		Email(s)
	}
}

// --- CERT_DIR ---
#[derive(Debug, Clone, Deserialize)]
pub struct CertDir(String);

impl Deref for CertDir {
	type Target = String;

	fn deref(&self) -> &String {
		&self.0
	}
}

impl fmt::Display for CertDir {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl From<String> for CertDir {
	fn from(s: String) -> Self {
		CertDir(s)
	}
}
