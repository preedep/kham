use crate::corpus::CorpusDb;
use crate::{collocate, freq, kwic};
use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::header,
    response::{Html, IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

struct AppState {
    db: Mutex<CorpusDb>,
    validator_url: Option<String>,
    http: reqwest::Client,
}

pub async fn serve(corpus_path: &str, port: u16, validator_url: Option<String>) -> Result<()> {
    let db = CorpusDb::open(corpus_path)?;

    if let Some(ref url) = validator_url {
        tracing::info!("NER validator: {url}");
    } else {
        tracing::info!("NER validator: not configured (pass --validator-url to enable)");
    }

    let state = Arc::new(AppState {
        db: Mutex::new(db),
        validator_url,
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?,
    });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/kwic", get(kwic_handler))
        .route("/api/freq", get(freq_handler))
        .route("/api/collocate", get(collocate_handler))
        .route("/api/corrections", get(corrections_list_handler))
        .route("/api/correct", post(correct_save_handler))
        .route("/api/correct", delete(correct_delete_handler))
        .route("/api/corrections/export", get(corrections_export_handler))
        .route("/api/suggest", get(suggest_handler))
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

#[derive(Deserialize)]
struct CorrectBody {
    word: String,
    correct_pos: Option<String>,
    correct_ne: Option<String>,
    note: Option<String>,
}

async fn correct_save_handler(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CorrectBody>,
) -> Json<serde_json::Value> {
    let db = s.db.lock().unwrap();
    match db.save_correction(
        &body.word,
        body.correct_pos.as_deref(),
        body.correct_ne.as_deref(),
        body.note.as_deref(),
    ) {
        Ok(()) => Json(serde_json::json!({ "ok": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct WordParam {
    word: String,
}

async fn correct_delete_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<WordParam>,
) -> Json<serde_json::Value> {
    let db = s.db.lock().unwrap();
    match db.delete_correction(&p.word) {
        Ok(()) => Json(serde_json::json!({ "ok": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct CorrectionsListParams {
    #[serde(default = "default_corrections_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

async fn corrections_list_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<CorrectionsListParams>,
) -> Json<serde_json::Value> {
    let db = s.db.lock().unwrap();
    match db.list_corrections(p.limit, p.offset) {
        Ok(rows) => Json(serde_json::json!({ "results": rows })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct ExportParams {
    #[serde(default = "default_export_format")]
    format: String,
}

async fn corrections_export_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<ExportParams>,
) -> Response {
    let db = s.db.lock().unwrap();
    match p.format.as_str() {
        "ne_tsv" => match db.export_corrections_ne() {
            Ok(rows) => {
                let body = rows
                    .into_iter()
                    .map(|(w, ne)| format!("{w}\t{ne}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                (
                    [
                        (
                            header::CONTENT_TYPE,
                            "text/tab-separated-values; charset=utf-8",
                        ),
                        (
                            header::CONTENT_DISPOSITION,
                            "attachment; filename=\"ne_corrections.tsv\"",
                        ),
                    ],
                    body,
                )
                    .into_response()
            }
            Err(e) => {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        },
        _ => match db.export_corrections_pos() {
            Ok(rows) => {
                let body = rows
                    .into_iter()
                    .map(|(w, pos)| format!("{w}\t{pos}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                (
                    [
                        (
                            header::CONTENT_TYPE,
                            "text/tab-separated-values; charset=utf-8",
                        ),
                        (
                            header::CONTENT_DISPOSITION,
                            "attachment; filename=\"pos_corrections.tsv\"",
                        ),
                    ],
                    body,
                )
                    .into_response()
            }
            Err(e) => {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        },
    }
}

// ── NER Suggest ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SuggestParams {
    word: String,
    #[serde(default)]
    context: String,
}

async fn suggest_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<SuggestParams>,
) -> Json<serde_json::Value> {
    let Some(ref base_url) = s.validator_url else {
        return Json(serde_json::json!({ "available": false }));
    };

    let url = format!("{base_url}/validate");
    match s
        .http
        .post(&url)
        .json(&serde_json::json!({ "word": p.word, "context": p.context }))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(mut v) => {
                v["available"] = serde_json::json!(true);
                Json(v)
            }
            Err(e) => {
                Json(serde_json::json!({ "available": false, "error": format!("parse: {e}") }))
            }
        },
        Err(e) => Json(serde_json::json!({ "available": false, "error": format!("connect: {e}") })),
    }
}

fn default_corrections_limit() -> usize {
    200
}

fn default_export_format() -> String {
    "pos_tsv".to_string()
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
