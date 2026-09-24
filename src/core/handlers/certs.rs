use crate::{
	Error, Result,
	config::{ACME_CHALLENGE_PREFIX, CERT_RENEWAL_TRESHOLD_DAYS},
	core::{
		handlers::filesystem::{check_file_exists, read_file, safe_path, write_file},
		models::{
			certs::{
				CertAccountPath, CertDir, CertPath, CertificateConfig, CertificateType, Email,
				KeyPath, OrderOutcome, TlsMaterial, TlsStore,
			},
			dns::{
				ChallengePrefix, Cloudflare, CloudflareProvider, DnsProvider, ProviderCredentail,
			},
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
use boring::asn1::Asn1Time;
use instant_acme::{
	Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, NewOrder, Order,
	OrderStatus, RetryPolicy,
};
use pingora::{server::ShutdownWatch, services::background::BackgroundService, tls};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use reqwest::Client;
use std::{collections::HashMap, fs, sync::Arc, time::Duration};
use tokio::time;

/// ------------------------------
/// Main background cert loop
/// ------------------------------
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

		// mutated in the renew_host to keep track of the order state
		let mut pending_order_urls: HashMap<Host, String> = HashMap::new();

		loop {
			info!("renewal loop: {} ACME hosts", configs.clone().count());

			for config in configs.clone() {
				if let Err(err) = renew_host(
					&self.tls_store,
					&account,
					&mut pending_order_urls,
					&http_client,
					config.clone(),
				)
				.await
				{
					error!("renewal failed for {}: {err:?}", config.host);
				}
			}

			info!("background renewal loop sleeping for {} seconds...", *self.task_interval);

			tokio::select! {
				_ = tokio::time::sleep(Duration::from_secs(*self.task_interval)) => {}
				_ = shutdown.changed() => break,
			}
		}
	}
}

// ------------------------------
// Cert functions used in our
// background cert loop
// ------------------------------

pub fn swap_store(store: &TlsStore, host: Host, key_bytes: &[u8], cert_bytes: &[u8]) -> Result<()> {
	let tls = parse_certificates(cert_bytes, key_bytes)?;

	let mut new_certs = HashMap::clone(&store.load());

	new_certs.insert(host, tls);
	store.store(Arc::new(new_certs));

	Ok(())
}

pub fn parse_certificates(cert_bytes: &[u8], key_bytes: &[u8]) -> Result<TlsMaterial> {
	let cert = tls::x509::X509::from_pem(cert_bytes)
		.map_err(|e| Error::Certificate(format!("Failed to parse certificate: {}", e)))?;

	let key = tls::pkey::PKey::private_key_from_pem(key_bytes)
		.map_err(|e| Error::Certificate(format!("Failed to parse private key: {}", e)))?;

	Ok(TlsMaterial {
		cert,
		key,
	})
}

pub fn certificate_paths(host: &Host, cert_dir: &CertDir) -> Result<(KeyPath, CertPath)> {
	let cert_filename = format!("{}.pem", host);
	let key_filename = format!("{}.key", host);

	let key_path = safe_path(cert_dir, &key_filename)?;
	let cert_path = safe_path(cert_dir, &cert_filename)?;

	Ok((key_path, cert_path))
}

async fn renew_host(
	tls_store: &TlsStore,
	account: &Account,
	pending_order_urls: &mut HashMap<Host, String>,
	http_client: &Client,
	config: CertificateConfig,
) -> Result<()> {
	// --------------
	// CHECK STAGE
	// --------------
	info!("[{}] checking", config.host);

	// check if dns provider is configured
	let dns_service_config = match config.provider_config.clone() {
		Some(config) => config,
		None => {
			warn!("Skipping Cert...");
			warn!("No DNS credentials provided for {}", config.host);
			return Ok(());
		},
	};
	// check if existing cert needs renewal
	let certs = tls_store.load();

	if let Some(TlsMaterial {
		cert,
		..
	}) = certs.get(config.host.as_str())
	{
		let threshold = Asn1Time::days_from_now(CERT_RENEWAL_TRESHOLD_DAYS)
			.map_err(|e| Error::Certificate(format!("Failed to create threshold time: {}", e)))?;

		let needs_renewal = cert.not_after() < threshold;

		if !needs_renewal {
			info!(
				"[{}] skip: cert valid until {}, threshold {} days",
				config.host,
				cert.not_after(),
				CERT_RENEWAL_TRESHOLD_DAYS
			);
			return Ok(());
		}
	};

	// --------------
	// CHOOSE STAGE
	// --------------
	let order_url = pending_order_urls.get(&config.host);
	let mut order = create_order(account, &config.host, order_url).await?;

	let ready_to_validate = order_url.is_some();

	if ready_to_validate {
		info!("[{}] stage 2: resuming order", config.host);
	} else {
		info!("[{}] stage 1: new order", config.host);
	}

	// --------------
	// ACTION STAGE
	// --------------
	let order_status = order.state().status;
	info!("[{}] order status {:?}", config.host, order_status);

	let dns_service =
		get_dns_services(dns_service_config, http_client.clone(), config.host.clone());

	let outcome = match (order_status, ready_to_validate) {
		(OrderStatus::Pending, false) => {
			authorizations_dns(&mut order, &dns_service, &config.host).await?;
			OrderOutcome::Waiting
		},
		(OrderStatus::Pending, true) => {
			authorizations_ready(&mut order, &config.host).await?;
			finish_order(&mut order, tls_store, &config).await?
		},
		(OrderStatus::Ready, _) => finish_order(&mut order, tls_store, &config).await?,
		(OrderStatus::Valid, _) => OrderOutcome::Dead,
		(OrderStatus::Processing, _) => OrderOutcome::Dead,
		(OrderStatus::Invalid, _) => OrderOutcome::Dead,
	};

	match outcome {
		OrderOutcome::Issued => {
			info!("[{}] renewed", config.host);
			pending_order_urls.remove(&config.host)
		},
		OrderOutcome::Dead => {
			warn!("[{}] order dropped ({:?}), new order next tick", config.host, order_status);
			pending_order_urls.remove(&config.host)
		},
		OrderOutcome::Waiting => {
			info!("[{}] holding order for next tick", config.host);
			pending_order_urls.insert(config.host.clone(), order.url().to_string())
		},
	};

	Ok(())
}

async fn finish_order(
	order: &mut Order,
	tls_store: &TlsStore,
	config: &CertificateConfig,
) -> Result<OrderOutcome> {
	// Exponentially back off until the order becomes ready or invalid.
	let status = order
		.poll_ready(&RetryPolicy::default())
		.await
		.map_err(|e| Error::Certificate(format!("Polling order failed: {}", e)))?;

	if status != OrderStatus::Ready {
		return Err(Error::Certificate(format!("Order is not ready: {:?}", status)));
	}

	info!("[{}] order ready, finalizing", config.host);

	let private_key_pem = order
		.finalize()
		.await
		.map_err(|e| Error::Certificate(format!("Finalizing order failed: {}", e)))?;

	let cert_chain_pem = order
		.poll_certificate(&RetryPolicy::default())
		.await
		.map_err(|e| Error::Certificate(format!("Polling certificate failed: {}", e)))?;

	info!("[{}] certificate issued", config.host);

	let (key_path, cert_path) = certificate_paths(&config.host, &config.cert_dir)?;

	write_file(key_path, private_key_pem.as_bytes())?;
	write_file(cert_path, cert_chain_pem.as_bytes())?;

	info!("[{}] cert and key written to {}", config.host, config.cert_dir);

	swap_store(
		tls_store,
		config.host.clone(),
		private_key_pem.as_bytes(),
		cert_chain_pem.as_bytes(),
	)?;

	info!("[{}] tls store updated", config.host);

	Ok(OrderOutcome::Issued)
}

async fn authorizations_dns(
	order: &mut Order,
	dns_service: &impl DnsProvider,
	host: &Host,
) -> Result<()> {
	info!("[{}] writing dns-01 challenge records", host);

	let mut authorizations = order.authorizations();

	while let Some(result) = authorizations.next().await {
		let mut authz = result
			.map_err(|e| Error::Certificate(format!("authorizations for this order: {}", e)))?;

		match authz.status {
			// all status should continue here, we are create a new entry
			AuthorizationStatus::Pending
			| AuthorizationStatus::Invalid
			| AuthorizationStatus::Revoked
			| AuthorizationStatus::Expired
			| AuthorizationStatus::Deactivated => {},
			AuthorizationStatus::Valid => {
				info!("[{}] authorization already valid, no record needed", host);
				continue;
			},
			// no catch all to prevent introducing new status
		}

		let mut challenge = match authz.challenge(ChallengeType::Dns01) {
			Some(challenge) => challenge,
			None => {
				return Err(Error::Certificate("no dns01 challenge found".to_string()));
			},
		};

		let dns_value = challenge.key_authorization().dns_value();
		let record = dns_service.upsert_challenge_record(dns_value).await?;

		info!("[{}] txt record {} = {}", host, record.name, record.value);
	}

	Ok(())
}

async fn authorizations_ready(order: &mut Order, host: &Host) -> Result<()> {
	info!("[{}] marking dns-01 challenges ready", host);

	let mut authorizations = order.authorizations();

	while let Some(result) = authorizations.next().await {
		let mut authz = result
			.map_err(|e| Error::Certificate(format!("authorizations for this order: {}", e)))?;

		match authz.status {
			// this one should continue
			AuthorizationStatus::Pending => {},
			// others need to just skip and log
			AuthorizationStatus::Valid
			| AuthorizationStatus::Expired
			| AuthorizationStatus::Invalid
			| AuthorizationStatus::Revoked
			| AuthorizationStatus::Deactivated => {
				info!("[{}] authorization {:?}", host, authz.status);
				continue;
			},
			// no catch all to prevent introducing new status
		}

		let mut challenge = match authz.challenge(ChallengeType::Dns01) {
			Some(challenge) => challenge,
			None => {
				return Err(Error::Certificate("no dns01 challenge found".to_string()));
			},
		};

		challenge
			.set_ready()
			.await
			.map_err(|e| Error::Certificate(format!("set challenge ready: {}", e)))?;

		info!("[{}] challenge marked ready, polling", host);
	}

	Ok(())
}

fn get_dns_services(config: ProviderCredentail, client: Client, host: Host) -> impl DnsProvider {
	match config {
		ProviderCredentail::Cloudflare(config) => Cloudflare {
			client,
			host,
			config: CloudflareProvider {
				zone_id: config.zone_id,
				api_token: config.api_token,
			},
			challenge_prefix: ChallengePrefix::from(ACME_CHALLENGE_PREFIX.to_string()),
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

// ------------------------------
// Self-signed cert functions
// those are used in the setup stage
// ------------------------------

pub fn create_self_signed_certs(certificate_configs: &Vec<CertificateConfig>) -> Result<()> {
	for config in certificate_configs {
		if config.cert_type == CertificateType::SelfSigned {
			// self signed certificates are good until the year 4096
			// this will be replace every restart so it's safe to keep using the default setting
			// for selfsigned we will create the certs here right away
			create_self_signed_certificate_files(config);
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
			let key_bytes = read_file(&key_path)?;

			let tls = parse_certificates(&cert_bytes, &key_bytes)?;

			tls_certs.insert(config.host.clone(), tls);
		}
	}

	let tls_store: TlsStore = Arc::new(ArcSwap::from_pointee(tls_certs));

	Ok(tls_store)
}
