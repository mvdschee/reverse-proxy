use crate::{
	core::models::{
		certs::{CertAccountPath, CertDir, Email},
		proxy::{ProxyInputAddress, ProxyPort},
		routes::Route,
		tasks::TaskInterval,
	},
	string_newtype,
};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Config {
	pub email: Email,
	pub cert_dir: CertDir,
	pub acme_env: AcmeEnv,
	// opague string type as it can't be cloned when its in AccountCredentials type
	pub cert_account_path: CertAccountPath,
	pub routes: Vec<Route>,
	pub task_interval_default: TaskInterval,
	pub task_interval_pending: TaskInterval,
	pub http_port: ProxyPort,
	pub https_port: ProxyPort,
	pub input_address: ProxyInputAddress,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigTomlFile {
	pub acme: Acme,
	pub routes: Vec<Route>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Acme {
	pub email: Email,
}

// --- CERT_DIR ---
string_newtype!(AcmeEnv);
