use std::sync::Arc;

use rusqlite::{named_params, params, Connection, OptionalExtension};
use tokio::sync::Mutex;


pub type Event = (i64, Option<i64>);

#[derive(Clone)]
pub struct Database(Arc<Mutex<Connection>>);

impl Database {
	pub fn open(path: Option<&str>) -> rusqlite::Result<Self> {
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

	pub async fn services(&self) -> rusqlite::Result<Vec<(i64, String)>> {
		let db = self.0.lock().await;
		let mut stmt = db.prepare("SELECT id, name FROM services")?;
		let res = stmt.query_map(
			params![],
			|row| Ok((row.get(0)?, row.get(1)?))
		)?;
		Ok(res.filter_map(|x| x.ok()).collect())
	}
	
	pub async fn insert(&self, sid: i64, value: Option<i64>) -> rusqlite::Result<()> {
		self.0.lock().await.execute(
			"INSERT INTO events(service, time, value) VALUES (:sid, :time, :value)",
			named_params! { ":sid": sid, ":time": chrono::Utc::now().timestamp(), ":value": value }
		)?;
		
		Ok(())
	}

	pub async fn get(&self, sid: i64, limit: i64) -> rusqlite::Result<Vec<Event>> {
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

	#[async_recursion::async_recursion] // TODO can we not???
	pub async fn sid(&self, service: &str, upsert: bool) -> rusqlite::Result<i64> {
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

	pub async fn up(&self, sid: i64, since: i64) -> rusqlite::Result<Option<i64>> {
		let db = self.0.lock().await;
		let mut stmt = db.prepare("SELECT value FROM events WHERE service = :sid AND time > :time")?;
		stmt.query_row(
			named_params! { ":sid": sid, ":time": since },
			|row| row.get::<usize, Option<i64>>(0)
		)
	}
}
