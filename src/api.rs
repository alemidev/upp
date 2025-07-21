use std::collections::HashMap;

use axum::{extract::{Path, Query, State}, response::{Html, IntoResponse}, Json};

use crate::{db::Database, Config};

pub async fn serve(config: Config, db: Database, addr: &str) -> std::io::Result<()>{
	// whats a jinja
	let index = include_str!("../web/index.html")
		.replacen("%%DESCRIPTION%%", config.description.as_deref().unwrap_or("keeping track of your infra's up status"), 1)
		.replacen("%%THRESHOLD%%", &config.threshold.unwrap_or(1000).to_string(), 1)
		.replacen("%%BATCHSIZE%%", &config.batchsize.unwrap_or(120).to_string(), 1);

	let app = axum::Router::new()
		.route("/", axum::routing::get(|| async { Html(index) }))
		.route("/favicon.ico", axum::routing::get(|| async { include_bytes!("../web/upp.ico") }))
		.route("/api/status", axum::routing::get(api_status))
		.route("/api/status/{service}", axum::routing::get(api_status_service))
		.with_state(db);

	let listener = tokio::net::TcpListener::bind(addr).await?;

	// TODO graceful shutdown
	// TODO maybe don't block here but allow parent to compose things?
	axum::serve(listener, app).await?;

	Ok(())
}

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

#[derive(serde::Deserialize)]
struct StatusQuery {
	since: Option<i64>,
}

async fn api_status(
	State(db): State<Database>,
	Query(q): Query<StatusQuery>,
) -> ApiResult<HashMap<String, Option<i64>>> {
	let mut state = HashMap::new();
	for (sid, name) in db.services().await? {
		if let Ok(up) = db.up(sid, q.since).await {
			state.insert(name, up);
		}
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
	let limit = q.limit.unwrap_or(50).min(300);
	let sid = db.sid(&service, false).await?;
	Ok(Json(db.get(sid, limit).await?))
}
