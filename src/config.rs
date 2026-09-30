use crate::{
	Error, Result,
	core::models::{
		certs::{CertAccountPath, CertDir},
		config::{AcmeEnv, Config, ConfigTomlFile},
		proxy::{ProxyInputAddress, ProxyPort},
		tasks::TaskInterval,
	},
};
use std::{env, fs};

const CONFIG_PATH_ENV: &str = "CONFIG_PATH";
const CERT_DIR_ENV: &str = "CERT_DIR";
const HTTP_PORT_ENV: &str = "HTTP_PORT";
const HTTPS_PORT_ENV: &str = "HTTPS_PORT";
const ACME_ENV: &str = "ACME_ENV";
pub const ACME_CHALLENGE_PREFIX: &str = "_acme-challenge.";

const CERT_DIR_DEFAULT: &str = ".certs/";
// this will be stored in the .certs/ or depending on where the user wants to store it
const CERT_CREDENTIAL_FILE: &str = "acme_account";
// set
const ACME_ENV_DEFAULT: &str = "production";
const HTTP_PORT_DEFAULT: u16 = 80;
const HTTPS_PORT_DEFAULT: u16 = 443;
const INPUT_ADDRESS: &str = "0.0.0.0";

/// in seconds
const CERT_BACKGROUND_TASK_INTERVAL: u64 = 3600; // 1 hour
pub const CERT_RENEWAL_TRESHOLD_DAYS: u32 = 30;

impl Config {
	pub fn init() -> Result<Self> {
		let config_path = load_env(CONFIG_PATH_ENV)?;
		let config_file = parse_toml_config(config_path)?;

		let cert_dir = load_env(CERT_DIR_ENV).unwrap_or_else(|_| CERT_DIR_DEFAULT.to_string());
		let cert_account_path = format!("{}/{}", cert_dir, CERT_CREDENTIAL_FILE);

		let http_port =
			load_env(HTTP_PORT_ENV).ok().and_then(|v| v.parse().ok()).unwrap_or(HTTP_PORT_DEFAULT);
		let https_port = load_env(HTTPS_PORT_ENV)
			.ok()
			.and_then(|v| v.parse().ok())
			.unwrap_or(HTTPS_PORT_DEFAULT);

		let acme_env = load_env(ACME_ENV).unwrap_or_else(|_| ACME_ENV_DEFAULT.to_string());

		Ok(Config {
			email: config_file.acme.email.clone(),
			acme_env: AcmeEnv::from(acme_env),
			cert_dir: CertDir::from(cert_dir),
			cert_account_path: CertAccountPath::from(cert_account_path),
			routes: config_file.routes.clone(),
			task_interval: TaskInterval::from(CERT_BACKGROUND_TASK_INTERVAL),
			http_port: ProxyPort::from(http_port),
			https_port: ProxyPort::from(https_port),
			input_address: ProxyInputAddress::from(INPUT_ADDRESS.to_string()),
		})
	}
}

fn parse_toml_config(config_path: String) -> Result<ConfigTomlFile> {
	let content = fs::read_to_string(config_path)?;
	let config: ConfigTomlFile =
		toml::from_str(&content).map_err(|e| Error::Config(e.to_string()))?;

	Ok(config)
}

fn load_env(key: &str) -> Result<String> {
	match env::var(key) {
		Ok(val) => Ok(val),
		Err(_err) => Err(Error::Env(key.to_string())),
	}
}
