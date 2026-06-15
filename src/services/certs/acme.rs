use crate::{
	Error, Result,
	core::models::certs::{CertificateConfig, Email},
	info,
};
use instant_acme::{Account, AccountCredentials, LetsEncrypt, NewAccount};
use rustls::crypto::CryptoProvider;

pub fn create_acme_dns_challenge(config: &CertificateConfig) -> Result<()> {
	info!("creating acme dns challenge for {}", config.host);

	Ok(())
}

pub async fn create_account(email: &Email) -> Result<(Account, AccountCredentials)> {
	CryptoProvider::install_default(rustls::crypto::ring::default_provider());

	let (account, credentials) = Account::builder()
		.map_err(|e| Error::Certificate(e.to_string()))?
		.create(
			&NewAccount {
				contact: &[&format!("mailto:{}", email.as_str())],
				terms_of_service_agreed: true,
				only_return_existing: false,
			},
			LetsEncrypt::Staging.url().to_owned(),
			None,
		)
		.await
		.map_err(|e| Error::Certificate(e.to_string()))?;

	Ok((account, credentials))
}
