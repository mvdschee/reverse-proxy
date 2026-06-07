use crate::{Error, Result, core::models::certs::CertificateConfig, info};

pub fn create_acme_dns_challenge(config: &CertificateConfig) -> Result<()> {
	info!("creating acme dns challenge for {}", config.host);

	Ok(())
}
