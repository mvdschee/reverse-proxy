use crate::{
	core::models::{filesystem::SafePath, routes::Host},
	string_newtype,
};
use arc_swap::ArcSwap;
use pingora::tls::{
	pkey::{PKey, Private},
	x509::X509,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CertificateType {
	SelfSigned,
	Acme,
	#[default]
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
string_newtype!(Email, derive(Deserialize));

// --- CERT_DIR ---
string_newtype!(CertDir, derive(Deserialize));
