use crate::{
	Error, Result,
	core::{
		handlers::filesystem::{check_file_exists, read_file, safe_path, write_file},
		models::{
			certs::{
				CertAccountPath, CertDir, CertPath, CertificateConfig, CertificateType, Email,
				KeyPath, TlsMaterial, TlsStore,
			},
			dns::{Cloudflare, CloudflareProvider, DnsProvider, ProviderCredentail},
			routes::Host,
			tasks::TaskInterval,
		},
	},
	error, info,
	services::{
		certs::{
			acme::{create_account, create_order, init_account, load_account},
			self_signed::create_self_signed_certificate_files,
		},
		http::create_client,
	},
	warn,
};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use instant_acme::{Account, AccountCredentials, Identifier, NewOrder, OrderStatus};
use pingora::{server::ShutdownWatch, services::background::BackgroundService, tls};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use std::{collections::HashMap, fs, sync::Arc, time::Duration};
use tokio::time;

pub fn create_self_signed_certs(certificate_configs: &Vec<CertificateConfig>) -> Result<()> {
	for config in certificate_configs {
		match config.cert_type {
			CertificateType::SelfSigned => {
				// self signed certificates are good until the year 4096
				// this will be replace every restart so it's safe to keep using the default setting
				// for selfsigned we will create the certs here right away
				create_self_signed_certificate_files(config);
			},
			_ => {},
		}
	}

	Ok(())
}

pub fn load_tls_store(certificate_configs: &Vec<CertificateConfig>) -> Result<TlsStore> {
	let mut tls_certs = HashMap::new();

	for config in certificate_configs {
		if config.cert_type != CertificateType::None {
			let (key_path, cert_path) = certificate_paths(&config.host, &config.cert_dir)?;

			let has_tls_files = check_file_exists(&key_path) && check_file_exists(&cert_path);

			// We only show a warning so its easier to debug once its running,
			// but we are not stopping any traffic.
			if !has_tls_files {
				warn!("Certificate files not found for host '{}' but is expected", &config.host);
				continue;
			}

			let cert_bytes = read_file(&cert_path)?;
			let cert = tls::x509::X509::from_pem(&cert_bytes)
				.map_err(|e| Error::Certificate(format!("Failed to parse certificate: {}", e)))?;

			let key_bytes = read_file(&key_path)?;
			let key = tls::pkey::PKey::private_key_from_pem(&key_bytes)
				.map_err(|e| Error::Certificate(format!("Failed to parse private key: {}", e)))?;

			tls_certs.insert(
				config.host.clone(),
				TlsMaterial {
					cert,
					key,
				},
			);
		}
	}

	let tls_store: TlsStore = Arc::new(ArcSwap::from_pointee(tls_certs));

	Ok(tls_store)
}

pub fn certificate_paths(host: &Host, cert_dir: &CertDir) -> Result<(KeyPath, CertPath)> {
	let cert_filename = format!("{}.pem", host);
	let key_filename = format!("{}.key", host);

	let key_path = safe_path(cert_dir, &key_filename)?;
	let cert_path = safe_path(cert_dir, &cert_filename)?;

	Ok((key_path, cert_path))
}

pub struct CertBackgroundRenewal {
	pub certificate_configs: Vec<CertificateConfig>,
	pub cert_account_path: CertAccountPath,
	pub task_interval: TaskInterval,
	pub tls_store: TlsStore,
	pub email: Email,
}

impl CertBackgroundRenewal {
	pub fn new(
		certificate_configs: Vec<CertificateConfig>,
		cert_account_path: CertAccountPath,
		task_interval: TaskInterval,
		tls_store: TlsStore,
		email: Email,
	) -> Self {
		Self {
			certificate_configs,
			cert_account_path,
			task_interval,
			tls_store,
			email,
		}
	}
}

#[async_trait]
impl BackgroundService for CertBackgroundRenewal {
	// start should never return this will stop the background task,
	// this means we have to be a little more verbose with our error handeling.
	// TLDR; just continue on any error :D, problem for the next loop :')
	async fn start(&self, mut shutdown: ShutdownWatch) {
		let http_client = match create_client() {
			Ok(client) => client,
			Err(e) => {
				error!("Can't start cert renewal loop: {}", e);
				return;
			},
		};

		let account = match resolve_acme_account(&self.cert_account_path, &self.email).await {
			Ok(account) => account,
			Err(err) => {
				error!("Failed to resolve ACME account: {err:?}");
				return;
			},
		};

		let configs = self
			.certificate_configs
			.clone()
			.into_iter()
			.filter(|c| c.cert_type == CertificateType::Acme);

		loop {
			for config in configs.clone() {
				let order_result = create_order(&account, &config.host).await;

				// if no DNS credentials are provided there is DNS validation.
				// we should terminate early on.
				let dns_service_config = match config.provider_config {
					Some(config) => config,
					None => {
						warn!("No DNS credentials provided for {}", config.host);
						continue;
					},
				};

				let dns_service = get_dns_services(dns_service_config);

				let mut order = match order_result {
					Ok(order) => order,
					Err(err) => {
						error!("{err:?}");
						continue;
					},
				};

				let state = order.state();
				info!("order state: {:#?}", state);

				// TODO what does pending means? can we use this to gate the refresh on it. like this will tell us its time to refresh the dns record.
				if !matches!(state.status, OrderStatus::Pending) {
					warn!("Skipping non-Pending order: {:?}", state.status);
					continue;
				}

				// TODO dont verify the value go straigh to update we are going to get get_challenge_record once we have put it and use it as a input check before finalizing

				// Pick the desired challenge type and prepare the response.

				// let mut authorizations = order.authorizations();
				// while let Some(result) = authorizations.next().await {
				// 	let mut authz = result?;
				// 	match authz.status {
				// 		AuthorizationStatus::Pending => {},
				// 		AuthorizationStatus::Valid => continue,
				// 		_ => todo!(),
				// 	}

				// 	// We'll use the DNS challenges for this example, but you could
				// 	// pick something else to use here.

				// 	let mut challenge = authz
				// 		.challenge(ChallengeType::Dns01)
				// 		.ok_or_else(|| anyhow::anyhow!("no dns01 challenge found"))?;

				// 	println!("Please set the following DNS record then press the Return key:");
				// 	println!(
				// 		"_acme-challenge.{} IN TXT {}",
				// 		challenge.identifier(),
				// 		challenge.key_authorization()?.dns_value()
				// 	);
				// 	io::stdin().read_line(&mut String::new())?;

				// 	challenge.set_ready().await?;
				// }

				// // Exponentially back off until the order becomes ready or invalid.

				// let status = order.poll_ready(&RetryPolicy::default()).await?;
				// if status != OrderStatus::Ready {
				// 	return Err(anyhow::anyhow!("unexpected order status: {status:?}"));
				// }

				// // Finalize the order and print certificate chain, private key and account credentials.

				// let private_key_pem = order.finalize().await?;
				// let cert_chain_pem = order.poll_certificate(&RetryPolicy::default()).await?;

				// info!("certificate chain:\n\n{cert_chain_pem}");
				// info!("private key:\n\n{private_key_pem}");

				// write to the file system
				//
				// swap the file content in the store with the new values if any
				//
				//
				// note: we write to the file system so we can pick the files up and load them in the store when we restart or bootup
				// this so we don't have to deal here with loading if the files are there (so we only have to check here if the order is invalid or valid and swap when its time)
				// so on boot we load all the tls certs from self-signed / acme and check in this flow it its valid or not and fix it with a swap.
			}

			info!("background thing");

			tokio::select! {
				_ = tokio::time::sleep(Duration::from_secs(*self.task_interval)) => {}
				_ = shutdown.changed() => break,
			}
		}
	}
}

fn get_dns_services(config: ProviderCredentail) -> impl DnsProvider {
	match config {
		ProviderCredentail::Cloudflare(config) => CloudflareProvider {
			zone_id: config.zone_id,
			api_token: config.api_token,
			challenge_prefix: config.challenge_prefix,
		},
	}
}

async fn resolve_acme_account(
	cert_account_path: &CertAccountPath,
	email: &Email,
) -> Result<Account> {
	// set crypto lib to load/create the account
	init_account();

	if let Ok(credentials) = get_acme_account(cert_account_path) {
		return load_account(credentials).await;
	}

	let (account, credentials) = create_account(email).await?;
	let content = serde_json::to_vec(&credentials)
		.map_err(|e| Error::Acme(format!("Failed to serialize ACME account: {}", e)))?;
	write_file(cert_account_path.clone(), &content)?;

	Ok(account)
}

// any erorr one this part we will simply create a new account
fn get_acme_account(cert_account_path: &CertAccountPath) -> Result<AccountCredentials> {
	let raw_content = read_file(cert_account_path)?;

	serde_json::from_slice(&raw_content)
		.map_err(|e| Error::Acme(format!("Failed to parse ACME account: {}", e)))
}
