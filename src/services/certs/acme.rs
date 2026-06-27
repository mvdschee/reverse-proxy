use crate::{
	Error, Result,
	core::models::{
		certs::{CertificateConfig, Email},
		routes::Host,
	},
};
use instant_acme::{
	Account, AccountCredentials, Identifier, LetsEncrypt, NewAccount, NewOrder, Order,
};
use rustls::crypto::CryptoProvider;

pub async fn create_account(email: &Email) -> Result<(Account, AccountCredentials)> {
	CryptoProvider::install_default(rustls::crypto::ring::default_provider());

	let (account, credentials) = Account::builder()
		.map_err(|e| Error::Acme(e.to_string()))?
		.create(
			&NewAccount {
				contact: &[&format!("mailto:{}", email.as_str())],
				terms_of_service_agreed: true,
				only_return_existing: false,
			},
			// TODO: switch to prod
			LetsEncrypt::Staging.url().to_owned(),
			None,
		)
		.await
		.map_err(|e| Error::Acme(e.to_string()))?;

	Ok((account, credentials))
}

pub async fn create_order(account: &Account, host: &Host) -> Result<Order> {
	let identifier = Identifier::Dns(host.to_string());
	// instant_acme support multiple host per order,
	// but we need to know which dns needs to be updated
	// which is why we split them up, this is maybe something we can refine in V2
	let identifiers = vec![identifier];
	let order_result = account
		.new_order(&NewOrder::new(&identifiers))
		.await
		.map_err(|e| Error::Acme(format!("Failed with new_order: {}", e)));

	order_result
}
