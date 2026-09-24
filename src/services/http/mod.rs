use crate::Result;
use reqwest::Client;
use std::time::Duration;

// simply return client no need for struct,
// or extras
pub fn create_client() -> Result<Client> {
	let client = Client::builder()
		.timeout(Duration::from_secs(30))
		.pool_idle_timeout(Duration::from_secs(90))
		.pool_max_idle_per_host(10)
		.build()?;

	Ok(client)
}
