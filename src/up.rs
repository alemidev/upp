use crate::{config::Config, db::Database};


pub async fn work(config: Config, db: Database) -> Result<(), rusqlite::Error> {
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
