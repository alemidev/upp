use std::{collections::HashMap, sync::Arc};

use clap::Parser;
use rusqlite::{named_params, params, Connection, OptionalExtension};
use tokio::sync::Mutex;

#[derive(Parser)]
struct Cli {
	/// path to storage file, if not given will operate in memory
	storage: Option<String>,

	/// path to config file
	#[arg(short, long, default_value = "uppe.toml")]
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
	//history: usize,

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

fn main() {
	let cli = Cli::parse();

	let raw_config = std::fs::read_to_string(&cli.config)
		.expect("could not open config file");

	let config = toml::from_str::<Config>(&raw_config)
		.expect("invalid config format");

	let db =  Database::open(cli.storage.as_deref())
		.expect("failed instantiating database");

	tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("could not create tokio runtime")
		.block_on(entry(cli, config, db))
		.expect("event loop terminated with error");
}

async fn entry(cli: Cli, config: Config, db: Database) -> Result<(), Box<dyn std::error::Error>> {
	let default_interval = config.interval_s;

	for (key, service) in config.service {
		let interval = service.interval_s.unwrap_or(default_interval);
		let db = db.clone();
		let sid = db.sid(&key, true).await?;

		tokio::spawn(async move {
			loop {
				let res = test_route(&service.endpoint).await;
				let value = match res {
					Ok(rtt) => Some(rtt),
					Err(e) => {
						eprintln!(" ?  error polling service {key}: {e} -- {e:?}");
						None
					},
				};
				if let Err(e) = db.insert(sid, value).await {
					eprintln!("[!] error inserting value in database: {e} -- {e:?}");
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
		.with_state(db);

	let listener = tokio::net::TcpListener::bind(&cli.addr).await?;
	axum::serve(listener, app).await?;

	Ok(())
}

async fn test_route(url: &str) -> reqwest::Result<i64> {
	let before = chrono::Utc::now();
	reqwest::get(url)
		.await?
		.error_for_status()?;
	let delta = chrono::Utc::now() - before;
	Ok(delta.num_milliseconds())
}


// ============= APIs


type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug, thiserror::Error)]
enum ApiError {
	#[error("error interacting with database: {0}")]
	Db(#[from] rusqlite::Error),
}

impl IntoResponse for ApiError {
	fn into_response(self) -> axum::response::Response {
		match self {
			ApiError::Db(error) => (
				axum::http::StatusCode::INTERNAL_SERVER_ERROR,
				Json(serde_json::json!({
					"error": "database",
					"message": format!("{error}"),
					"struct": format!("{error:?}"),
				}))
			).into_response(),
		}
	}
}

async fn root() -> Html<&'static str> {
	Html(include_str!("../index.html"))
}

use axum::{extract::{Path, Query, State}, response::{Html, IntoResponse}, Json};

#[derive(serde::Deserialize)]
struct StatusQuery {
	since: Option<i64>,
}

async fn api_status(
	State(db): State<Database>,
	Query(q): Query<StatusQuery>,
) -> ApiResult<HashMap<String, Option<i64>>> {
	let mut state = HashMap::new();
	let five_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(5)).timestamp();
	let since = q.since.unwrap_or(five_min_ago);
	for (sid, name) in db.services().await? {
		state.insert(
			name,
			db.up(sid, since).await?
		);
	}
	Ok(Json(state))
}

#[derive(serde::Deserialize)]
struct ServiceStatusQuery {
	limit: Option<i64>,
}

async fn api_status_service(
	State(db): State<Database>,
	Path(service): axum::extract::Path<String>,
	Query(q): Query<ServiceStatusQuery>,
) -> ApiResult<Vec<(i64, Option<i64>)>> {
	let limit = q.limit.unwrap_or(50).min(250);
	let sid = db.sid(&service, false).await?;
	Ok(Json(db.get(sid, limit).await?))
}


// ============= DATABASE


type Event = (i64, Option<i64>);

#[derive(Clone)]
struct Database(Arc<Mutex<Connection>>);

impl Database {
	fn open(path: Option<&str>) -> rusqlite::Result<Self> {
		let db = match path {
			Some(p) => Connection::open(p)?,
			None => Connection::open_in_memory()?,
		};
		db.execute(
			"CREATE TABLE IF NOT EXISTS events (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				service INTEGER NOT NULL,
				time BIGINT NOT NULL,
				value BIGINT NULL
			)", params![]
		)?;

		db.execute(
			"CREATE INDEX IF NOT EXISTS event_per_service
			ON events (service)",
			params![],
		)?;
	
		db.execute(
			"CREATE TABLE IF NOT EXISTS services (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				name STRING NOT NULL
			)", params![]
		)?;

		db.execute(
			"CREATE INDEX IF NOT EXISTS services_names_lookup
			ON services (name)",
			params![],
		)?;

		Ok(Self(Arc::new(Mutex::new(db))))
	}

	async fn services(&self) -> rusqlite::Result<Vec<(i64, String)>> {
		let db = self.0.lock().await;
		let mut stmt = db.prepare("SELECT id, name FROM services")?;
		let res = stmt.query_map(
			params![],
			|row| Ok((row.get(0)?, row.get(1)?))
		)?;
		Ok(res.filter_map(|x| x.ok()).collect())
	}
	
	async fn insert(&self, sid: i64, value: Option<i64>) -> rusqlite::Result<()> {
		self.0.lock().await.execute(
			"INSERT INTO events(service, time, value) VALUES (:sid, :time, :value)",
			named_params! { ":sid": sid, ":time": chrono::Utc::now().timestamp(), ":value": value }
		)?;
		
		Ok(())
	}

	async fn get(&self, sid: i64, limit: i64) -> rusqlite::Result<Vec<Event>> {
		let db = self.0.lock().await;
		let mut stmt = db.prepare("SELECT time, value FROM events WHERE service = :sid LIMIT :limit")?;
		let results = stmt.query_map(
			named_params! { ":sid": sid, ":limit": limit },
			|row| Ok((row.get(0)?, row.get(1)?)),
		)?;

		Ok(
			results
				.filter_map(|x| x.ok())
				.collect()
		)
	}

	#[async_recursion::async_recursion]
	async fn sid(&self, service: &str, upsert: bool) -> rusqlite::Result<i64> {
		let res = {
			let db = self.0.lock().await;
			let mut stmt = db.prepare("SELECT id FROM services WHERE name = ?")?;
			stmt.query_row(params![service], |row| row.get(0)).optional()?
		};

		match res {
			Some(sid) => Ok(sid),
			None => {
				if upsert {
					self.0.lock().await.execute("INSERT INTO services(name) VALUES (?)", params![service])?;
					self.sid(service, upsert).await
				} else {
					Err(rusqlite::Error::QueryReturnedNoRows)
				}
			}
		}
	}

	async fn up(&self, sid: i64, since: i64) -> rusqlite::Result<Option<i64>> {
		let db = self.0.lock().await;
		let mut stmt = db.prepare("SELECT value FROM events WHERE service = :sid AND time > :time")?;
		stmt.query_row(
			named_params! { ":sid": sid, ":time": since },
			|row| row.get::<usize, Option<i64>>(0)
		)
	}
}
