use crate::{
	Error, Result,
	core::{
		handlers::filesystem::{check_file_exists, read_file, safe_path, write_file},
		models::{
			certs::{
				CertDir, CertPath, CertificateConfig, CertificateType, Email, KeyPath, TlsMaterial,
				TlsStore,
			},
			routes::Host,
			tasks::TaskInterval,
		},
	},
	error, info,
	services::certs::{
		acme::{create_account, create_order},
		self_signed::create_self_signed_certificate_files,
	},
	warn,
};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use instant_acme::{Identifier, NewOrder, OrderStatus};
use pingora::{server::ShutdownWatch, services::background::BackgroundService, tls};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use std::{collections::HashMap, sync::Arc, time::Duration};
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
	pub task_interval: TaskInterval,
	pub tls_store: TlsStore,
	pub email: Email,
}

impl CertBackgroundRenewal {
	pub fn new(
		certificate_configs: Vec<CertificateConfig>,
		task_interval: TaskInterval,
		tls_store: TlsStore,
		email: Email,
	) -> Self {
		Self {
			certificate_configs,
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
		// TODO what to do when creating an account fails (acme endpoints is 500 etc..)
		let account_result = create_account(&self.email)
			.await
			.map_err(|e| Error::Certificate(format!("Failed with create_account: {}", e)));

		let configs = self
			.certificate_configs
			.clone()
			.into_iter()
			.filter(|c| c.cert_type == CertificateType::Acme);

		loop {
			let (account, credentials) = match account_result {
				Ok(ref pair) => pair,
				Err(ref err) => {
					error!("{err:?}");
					continue;
				},
			};

			for config in configs.clone() {
				let order_result = create_order(&account, &config.host).await;

				let mut order = match order_result {
					Ok(order) => order,
					Err(err) => {
						error!("{err:?}");
						continue;
					},
				};

				let state = order.state();
				info!("order state: {:#?}", state);

				if !matches!(state.status, OrderStatus::Pending) {
					warn!("Skipping non-Pending order: {:?}", state.status);
					continue;
				}

				//
				// if so verify the dns records with cloudflare
				//
				// if its set allow the order to be proccessed
				//
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
