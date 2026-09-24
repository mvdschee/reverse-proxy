use crate::{
	Error, Result,
	core::{
		handlers::{
			certs::certificate_paths,
			filesystem::{safe_path, write_file},
		},
		models::certs::CertificateConfig,
	},
	info,
};
use rcgen::{CertifiedKey, generate_simple_self_signed};

pub fn create_self_signed_certificate_files(config: &CertificateConfig) -> Result<()> {
	info!("generating self-signed certificate for {}", config.host);

	let subject_alt_names = vec![config.host.to_string()];

	let (key_path, cert_path) = certificate_paths(&config.host, &config.cert_dir)?;

	let CertifiedKey {
		cert,
		signing_key,
	} = generate_simple_self_signed(subject_alt_names)
		.map_err(|e| Error::Certificate(e.to_string()))?;

	let cert_serialized = cert.pem();
	let key_serialized = signing_key.serialize_pem();

	write_file(cert_path, cert_serialized.as_bytes())?;
	write_file(key_path, key_serialized.as_bytes())?;

	Ok(())
}
