use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    extract::State,
    response::{Html, Json},
    routing::get,
    Router,
};
use ocrs::{OcrEngine, OcrEngineParams};
use tower_http::services::ServeDir;

/// Given a file path relative to the crate root, return the absolute path.
fn file_path(path: &str) -> PathBuf {
    let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    abs_path.push(path);
    abs_path
}

struct AppState {
    engine: OcrEngine,
}

async fn index() -> Html<String> {
    let index_html = tokio::fs::read_to_string("templates/index.html")
        .await
        .expect("Failed to read index.html");
    Html(index_html)
}

async fn process_test_pdf(State(state): State<Arc<AppState>>) -> Json<Vec<Vec<String>>> {
    let test_pdf_path = file_path("test2.pdf");
    
    // Process PDF and get text from all pages
    let texts = ocr_app::process_pdf(&state.engine, &test_pdf_path)
        .expect("Failed to process PDF");
    
    Json(texts)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize OCR engine with models
    let detection_model_path = file_path("models/text-detection-checkpoint-03.23.recall_92.precis_85.rten");
    let rec_model_path = file_path("models/text-rec-checkpoint-7.rten");

    // Load models
    let detection_model = ocr_app::models::load_model(detection_model_path.to_str().unwrap())
        .context("Failed to load detection model")?;
    let recognition_model = ocr_app::models::load_model(rec_model_path.to_str().unwrap())
        .context("Failed to load recognition model")?;

    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        debug: true,
        decode_method: ocrs::DecodeMethod::BeamSearch { width: 5 },
        ..Default::default()
    }).map_err(|e| anyhow::anyhow!("Failed to initialize OCR engine: {}", e))?;

    // Create app state
    let state = Arc::new(AppState { engine });

    // Create router
    let app = Router::new()
        .route("/", get(index))
        .route("/process-test-pdf", get(process_test_pdf))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    // Start server
    println!("Server running on http://localhost:3000");
    let addr: std::net::SocketAddr = "0.0.0.0:3000".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
        .context("Server error")?;

    Ok(())
}
