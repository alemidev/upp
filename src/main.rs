
use std::{collections::{HashMap, VecDeque}, sync::Arc};

use clap::Parser;
use tokio::sync::RwLock;

#[derive(Parser)]
struct Cli {
	/// path to config file
	#[arg(short, long, default_value = "uppe-rs.toml")]
	config: String,

	/// host to bind api onto
	#[arg(short, long, default_value = "127.0.0.1:7717")]
	addr: String,
}

#[derive(serde::Deserialize)]
struct Config {
	/// defined services, singular because makes more sense in toml
	service: std::collections::BTreeMap<String, Service>,

	/// how many samples of history to keep
	history: usize,

	/// poll services at this interval
	interval_s: u64,
}

#[derive(serde::Deserialize)]
struct Service {
	/// url to query
	endpoint: String,

	/// override poll rate for this service
	interval_s: Option<u64>,
}

type AppState = Arc<RwLock<StateStorage>>;

type Event = (i64, Option<i64>);

struct StateStorage {
	size: usize,
	store: HashMap<String, VecDeque<Event>>,
}

impl StateStorage {
	fn new(size: usize) -> AppState {
		Arc::new(RwLock::new(Self {
			size, store: HashMap::default(),
		}))
	}

	fn get(&self, k: &str) -> Vec<Event> {
		match self.store.get(k) {
			Some(x) => x.clone().into(),
			None => Vec::new(),
		}
	}

	fn put(&mut self, key: &str, timestamp: i64, rtt: Option<i64>) {
		match self.store.get_mut(key) {
			Some(x) => {
				x.push_back((timestamp, rtt));
				while x.len() > self.size {
					x.pop_front();
				}
			},
			None => {
				let mut q = VecDeque::new();
				q.push_back((timestamp, rtt));
				self.store.insert(key.to_string(), q);
			},
		}
	}

	fn up(&self, key: &str) -> bool {
		match self.store.get(key) {
			None => false, // this key is not being tracked, or we never polled it
			Some(x) => match x.back() {
				None => false, // this key has never been polled yet
				Some((_, None)) => false, // last poll was a failure
				Some((_, Some(_))) => true, // last poll was a success
			}
		}
	}

	fn services(&self) -> Vec<String> {
		self.store.keys().cloned().collect()
	}
}

fn main() {
	let cli = Cli::parse();

	let raw_config = std::fs::read_to_string(&cli.config)
		.expect("could not open config file");

	let config = toml::from_str::<Config>(&raw_config)
		.expect("invalid config format");

	tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("could not create tokio runtime")
		.block_on(entry(cli, config))
		.expect("event loop terminated with error");
}

async fn entry(cli: Cli, config: Config) -> Result<(), Box<dyn std::error::Error>> {
	let state = StateStorage::new(config.history);
	let default_interval = config.interval_s;

	for (key, service) in config.service {
		let interval = service.interval_s.unwrap_or(default_interval);
		let state = state.clone();

		tokio::spawn(async move {
			loop {
				let res = test(&service.endpoint).await;
				let timestamp = chrono::Utc::now().timestamp();
				match res {
					Ok(rtt) => state.write().await.put(&key, timestamp, Some(rtt)),
					Err(e) => {
						eprintln!("[!] error polling service {key}: {e}");
						state.write().await.put(&key, timestamp, None);
					},
				}
				tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
			}
		});
	}

	// build our application with a single route
	let app = axum::Router::new()
		.route("/", axum::routing::get(root))
		.route("/api/status", axum::routing::get(api_status))
		.route("/api/status/:service", axum::routing::get(api_status_service))
		.with_state(state);

	let listener = tokio::net::TcpListener::bind(&cli.addr).await?;
	axum::serve(listener, app).await?;

	Ok(())
}

async fn root() -> Html<&'static str> {
	Html(include_str!("../index.html"))
}

async fn test(url: &str) -> reqwest::Result<i64> {
	let before = chrono::Utc::now();
	reqwest::get(url)
		.await?
		.error_for_status()?;
	let delta = chrono::Utc::now() - before;
	Ok(delta.num_milliseconds())
}

use axum::{extract::{Path, State}, response::Html, Json};

async fn api_status(
	State(state): State<AppState>,
) -> Json<HashMap<String, bool>> {
	let services = state.read().await.services();
	let mut out = HashMap::new();
	for service in services {
		let up = state.read().await.up(&service);
		out.insert(service, up);
	}
	Json(out)
}

async fn api_status_service(
	State(state): State<AppState>,
	Path(service): axum::extract::Path<String>,
) -> Json<Vec<(i64, Option<i64>)>> {
	Json(state.read().await.get(&service))
}
