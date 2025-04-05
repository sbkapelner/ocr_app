use std::path::PathBuf;
use std::sync::Arc;
use std::io::Cursor;

use anyhow::{Context, Result};
use axum::{
    extract::{Multipart, State},
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use ocrs::{OcrEngine, OcrEngineParams};
use tower_http::services::ServeDir;
use tempfile::NamedTempFile;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ocr_app::OcrResult;

#[derive(serde::Serialize)]
struct ProcessResponse {
    pages: Vec<PageResult>,
}

#[derive(serde::Serialize)]
struct DocxProcessResponse {
    numbers: Vec<String>,
}

#[derive(serde::Serialize)]
struct PageResult {
    image: String,  // Base64 encoded image
    ocr_results: Vec<OcrResult>,
}

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

async fn process_docx(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<DocxProcessResponse>, String> {
    // Get the DOCX file from the form data
    let field = multipart
        .next_field()
        .await
        .map_err(|e| format!("Failed to get form field: {}", e))?;

    if field.is_none() {
        return Err("No file provided".to_string());
    }

    let field = field.unwrap();
    if field.name() != Some("docx") {
        return Err("Invalid form field name".to_string());
    }

    // Read the file data
    let data = field
        .bytes()
        .await
        .map_err(|e| format!("Failed to read file data: {}", e))?;

    // Create a temporary file
    let mut temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;

    // Write the data to the temporary file
    std::io::Write::write_all(&mut temp_file, &data)
        .map_err(|e| format!("Failed to write to temporary file: {}", e))?;

    // Process the DOCX
    let results = ocr_app::process_docx(&state.engine, temp_file.path())
        .map_err(|e| format!("Failed to process DOCX: {}", e))?;

    // Return the results
    Ok(Json(DocxProcessResponse { 
        numbers: results.numbers
    }))
}

async fn process_pdf(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<ProcessResponse>, String> {
    // Get the PDF file from the form data
    let field = multipart
        .next_field()
        .await
        .map_err(|e| format!("Failed to get form field: {}", e))?;

    if field.is_none() {
        return Err("No file provided".to_string());
    }

    let field = field.unwrap();
    if field.name() != Some("pdf") {
        return Err("Invalid form field name".to_string());
    }

    // Read the file data
    let data = field
        .bytes()
        .await
        .map_err(|e| format!("Failed to read file data: {}", e))?;

    // Create a temporary file
    let mut temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;

    // Write the data to the temporary file
    std::io::Write::write_all(&mut temp_file, &data)
        .map_err(|e| format!("Failed to write to temporary file: {}", e))?;

    // Process the PDF
    let results = ocr_app::process_pdf(&state.engine, temp_file.path())
        .map_err(|e| format!("Failed to process PDF: {}", e))?;

    // Convert results to response format
    let pages = results.into_iter().map(|(img, ocr_results)| {
        // Convert image to base64
        let mut img_data = Vec::new();
        img.write_to(&mut Cursor::new(&mut img_data), image::ImageOutputFormat::Png)
            .map_err(|e| format!("Failed to encode image: {}", e))?;
        let img_base64 = STANDARD.encode(&img_data);

        Ok(PageResult {
            image: format!("data:image/png;base64,{}", img_base64),
            ocr_results,
        })
    }).collect::<Result<Vec<_>, String>>()?;

    // Return the results
    Ok(Json(ProcessResponse { pages }))
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
        .route("/process-pdf", post(process_pdf))
        .route("/process-docx", post(process_docx))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    // Start server
    println!("Server running on http://localhost:3001");
    let addr: std::net::SocketAddr = "0.0.0.0:3001".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
        .context("Server error")?;

    Ok(())
}
