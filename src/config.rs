
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
	/// defined services, singular because makes more sense in toml
	pub service: std::collections::BTreeMap<String, Service>,

	/// service description shown in web page
	pub description: Option<String>,

	/// requests taking longer than this limit (in ms) will be marked as "slow" in FE
	pub threshold: Option<u64>,

	// TODO reintroduce this! should allow to optionally trim db periodically
	/// how many samples of history to keep
	//history: usize,

	/// poll services at this interval
	pub interval_s: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Service {
	/// url to query
	pub endpoint: String,

	/// override poll rate for this service
	pub interval_s: Option<u64>,
}
