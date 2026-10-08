use crate::{
	Error, Result,
	core::models::{dns::ChallengePrefix, routes::Host},
	services::dns::utils::create_txt_key,
};
use hickory_resolver::{Resolver, config::*, net::runtime::TokioRuntimeProvider};

pub struct DnsResolver {
	pub resolver: Resolver<TokioRuntimeProvider>,
	pub challenge_prefix: ChallengePrefix,
}

impl DnsResolver {
	pub fn new(challenge_prefix: ChallengePrefix) -> Result<Self> {
		// Construct a new Resolver with default configuration options
		let resolver = Resolver::builder_with_config(
			ResolverConfig::udp_and_tcp(&CLOUDFLARE),
			TokioRuntimeProvider::default(),
		)
		.build()
		.map_err(|e| Error::Dns(e.to_string()))?;

		Ok(Self {
			resolver,
			challenge_prefix,
		})
	}

	pub async fn lookup_text(&self, host: &Host) -> Result<Vec<String>> {
		let record_key = create_txt_key(host, &self.challenge_prefix);

		let response =
			self.resolver.txt_lookup(record_key).await.map_err(|e| Error::Dns(e.to_string()))?;

		let mut records = vec![];

		for record in response.message().answers.iter() {
			records.push(record.data.to_string());
		}

		Ok(records)
	}
}
