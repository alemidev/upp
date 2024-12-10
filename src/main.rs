mod db;
mod api;
mod config;
mod up;

use config::Config;
use db::Database;

use clap::Parser;


#[derive(Parser)]
struct Cli {
	/// path to storage file, if not given will operate in memory
	storage: Option<String>,

	/// path to config file
	#[arg(short, long, default_value = "upp.toml")]
	config: String,

	/// host to bind api onto
	#[arg(short, long, default_value = "127.0.0.1:7717")]
	addr: String,
}

fn main() {
	let cli = Cli::parse();

	let raw_config = match std::fs::read_to_string(&cli.config) {
		Ok(x) => x,
		Err(e) => {
			println!("could not read config: {e}");
			return;
		},
	};

	let config = match toml::from_str::<Config>(&raw_config) {
		Ok(x) => x,
		Err(e) => {
			println!("invalid config file: {e}");
			return;
		},
	};

	let db =  match Database::open(cli.storage.as_deref()) {
		Ok(x) => x,
		Err(e) => {
			println!("could not connect do database: {e}");
			return;
		},
	};

	let mut runtime_builder = {
		#[cfg(feature = "multi-thread")]
		{
			tokio::runtime::Builder::new_multi_thread()
		}

		#[cfg(not(feature = "multi-thread"))]
		{
			tokio::runtime::Builder::new_current_thread()
		}
	};

	if let Err(e) = runtime_builder
		.enable_all()
		.build()
		.expect("could not create tokio runtime")
		.block_on(async move {
			up::work(config.clone(), db.clone()).await?; // <<-- this spawns background workers

			api::serve(config, db, &cli.addr).await?; // <<-- this blocks!

			// TODO it's a bit weird that these two work so differently, can we make them more similar?

			Ok::<(), Box<dyn std::error::Error>>(()) // ughhh
		})
	{
		println!("event loop terminated with error: {e}");
		eprintln!("{e:?}");
	}
}
