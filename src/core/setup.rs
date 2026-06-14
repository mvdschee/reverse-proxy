use crate::{
	Error, Result,
	core::{
		handlers::{
			certs::{
				CertBackgroundRenewal, certificate_paths, create_self_signed_certs, load_tls_store,
			},
			filesystem::{check_file_exists, safe_path},
			proxy::run_proxy,
		},
		models::{
			certs::{CertDir, CertificateConfig, CertificateType, Email, TlsStore},
			proxy::{ProxyConfig, ProxyRoute, ProxyTls},
			routes::Route,
			tasks::TaskInterval,
		},
	},
	error, info, warn,
};
use std::path::Path;

pub struct HandleFileSystem {
	cert_dir: CertDir,
}

impl HandleFileSystem {
	pub fn new(cert_dir: CertDir) -> Self {
		Self {
			cert_dir,
		}
	}

	pub fn run(&self) -> Result<()> {
		info!("setup filesystem...");

		// foxguard: ignore[rs/no-path-traversal]
		// no user input is used here, so we can safely use Path::new directly
		if !Path::new(self.cert_dir.as_str()).exists() {
			info!("creating cert directory at {}", &self.cert_dir);
			std::fs::create_dir_all(self.cert_dir.as_str())?;
		} else {
			info!("cert directory already exists at {}", &self.cert_dir);
		}

		Ok(())
	}
}

pub struct HandleCertificates {
	certificate_configs: Vec<CertificateConfig>,
	task_interval: TaskInterval,
}

impl HandleCertificates {
	pub fn new(
		cert_dir: CertDir,
		email: Email,
		routes: Vec<Route>,
		task_interval: TaskInterval,
	) -> Self {
		let certificate_configs = routes
			.into_iter()
			.map(|route| CertificateConfig {
				host: route.host.clone(),
				cert_dir: cert_dir.clone(),
				email: email.clone(),
				cert_type: route.cert_type.clone(),
			})
			.collect::<Vec<CertificateConfig>>();

		Self {
			certificate_configs,
			task_interval,
		}
	}

	pub fn run(self) -> Result<(TlsStore, CertBackgroundRenewal)> {
		create_self_signed_certs(&self.certificate_configs)?;

		let store = load_tls_store(&self.certificate_configs)?;
		let renewal = CertBackgroundRenewal::new(
			self.certificate_configs.clone(),
			self.task_interval.clone(),
			store.clone(),
		);

		Ok((store, renewal))
	}
}

pub struct HandleProxy {
	proxy_config: ProxyConfig,
	proxy_routes: Vec<ProxyRoute>,
	tls_store: TlsStore,
}

impl HandleProxy {
	pub fn new(proxy_config: ProxyConfig, routes: Vec<Route>, tls_store: TlsStore) -> Result<Self> {
		let mut proxy_routes = Vec::new();

		for route in routes {
			proxy_routes.push(ProxyRoute {
				host: route.host.clone(),
				upstream: route.upstream.clone(),
				tls: ProxyTls::from(route.cert_type != CertificateType::None),
			});
		}

		Ok(Self {
			proxy_config,
			proxy_routes,
			tls_store,
		})
	}

	pub fn run(&self, renewal: CertBackgroundRenewal) -> Result<()> {
		info!("proxy running...");

		run_proxy(
			self.proxy_config.clone(),
			self.proxy_routes.clone(),
			self.tls_store.clone(),
			renewal,
		)?;

		Err(Error::Proxy("proxy exited".to_string()))
	}
}
