use crate::corpus::CorpusDb;
use crate::{collocate, freq, kwic};
use anyhow::Result;
use axum::{
    extract::{Query, State},
    response::{Html, Json},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

struct AppState {
    db: Mutex<CorpusDb>,
}

pub async fn serve(corpus_path: &str, port: u16) -> Result<()> {
    let db = CorpusDb::open(corpus_path)?;
    let state = Arc::new(AppState { db: Mutex::new(db) });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/kwic", get(kwic_handler))
        .route("/api/freq", get(freq_handler))
        .route("/api/collocate", get(collocate_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("kham-tnc listening on http://{addr}");
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn stats_handler(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let db = s.db.lock().unwrap();
    match db.corpus_stats() {
        Ok(stats) => Json(serde_json::json!(stats)),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct KwicParams {
    word: String,
    #[serde(default = "default_context")]
    context: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

async fn kwic_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<KwicParams>,
) -> Json<serde_json::Value> {
    let db = s.db.lock().unwrap();
    match kwic::search(&db, &p.word, p.context, p.limit, p.offset) {
        Ok(lines) => Json(serde_json::json!({ "results": lines })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct FreqParams {
    pos: Option<String>,
    ne: Option<String>,
    #[serde(default = "default_min_freq")]
    min_freq: i64,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

async fn freq_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<FreqParams>,
) -> Json<serde_json::Value> {
    let db = s.db.lock().unwrap();
    match freq::word_frequency(
        &db,
        p.pos.as_deref(),
        p.ne.as_deref(),
        p.min_freq,
        p.limit,
        p.offset,
    ) {
        Ok(entries) => Json(serde_json::json!({ "results": entries })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct CollocateParams {
    word: String,
    #[serde(default = "default_span")]
    left: usize,
    #[serde(default = "default_span")]
    right: usize,
    #[serde(default = "default_min_freq")]
    min_freq: i64,
}

async fn collocate_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<CollocateParams>,
) -> Json<serde_json::Value> {
    let db = s.db.lock().unwrap();
    match collocate::collocates(&db, &p.word, p.left, p.right, p.min_freq) {
        Ok(entries) => Json(serde_json::json!({ "results": entries })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

fn default_context() -> usize {
    5
}
fn default_limit() -> usize {
    50
}
fn default_span() -> usize {
    5
}
fn default_min_freq() -> i64 {
    2
}
