use crate::core::models::{dns::ChallengePrefix, routes::Host};

// create a dns record for the challenge
pub fn create_txt_key(host: &Host, challenge_prefix: &ChallengePrefix) -> String {
	let challenge = format!("{}{}", challenge_prefix, host);

	challenge
}
