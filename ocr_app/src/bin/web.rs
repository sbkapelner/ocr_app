use std::path::PathBuf;
use std::sync::Arc;
use std::io::Cursor;
use axum::extract::DefaultBodyLimit;
use sha2::{Sha256, Digest};

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

// Helper function to normalize a number
fn normalize_number(num: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = num.chars().collect();
    let len = chars.len();
    
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            result.push(*c);
        } else if *c == '-' && i > 0 && i < len - 1 && 
                  chars[i-1].is_ascii_digit() && chars[i+1].is_ascii_digit() {
            // Only keep hyphen if it's between two digits
            result.push(*c);
        }
    }
    result
}

#[derive(serde::Serialize)]
struct ProcessResponse {
    pages: Vec<PageResult>,
    file_hash: String,
}

#[derive(serde::Serialize)]
struct DocxProcessResponse {
    matches: Vec<String>,     // Full matches like "word 123"
    numbers: Vec<String>,     // Just the numbers for comparison
    html_content: String,
    file_hash: String,
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

async fn comparison_view() -> Html<String> {
    let template_path = "templates/comparison.html";
    match tokio::fs::read_to_string(template_path).await {
        Ok(content) => Html(content),
        Err(e) => Html(format!("Error reading comparison.html: {}", e))
    }
}

async fn index() -> Html<String> {
    println!("[DEBUG] Index route called");
    let index_path = "templates/index.html";
    println!("[DEBUG] Looking for index.html at: {}", index_path);
    println!("[DEBUG] Current working directory: {}", std::env::current_dir().unwrap().display());
    println!("[DEBUG] Directory contents:");
    if let Ok(entries) = std::fs::read_dir("templates") {
        for entry in entries {
            if let Ok(entry) = entry {
                println!("[DEBUG] - {}", entry.path().display());
            }
        }
    } else {
        println!("[DEBUG] Could not read /app/templates directory");
    }
    
    match tokio::fs::read_to_string(index_path).await {
        Ok(content) => {
            println!("[DEBUG] Successfully read index.html ({} bytes)", content.len());
            Html(content)
        }
        Err(e) => {
            println!("[DEBUG] Error reading index.html: {}", e);
            Html(format!("Error reading index.html: {}", e))
        }
    }
}

async fn process_docx(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<DocxProcessResponse>, String> {
    println!("[DEBUG] Starting DOCX processing");
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

    // Calculate SHA-256 hash
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = format!("{:x}", hasher.finalize());

    // Create a temporary file
    let temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;

    // Write the data to the temporary file
    std::io::Write::write_all(&mut temp_file.as_file(), &data)
        .map_err(|e| format!("Failed to write to temporary file: {}", e))?;

    let file_path = temp_file.path();

    // Process the DOCX
    println!("[DEBUG] Processing DOCX file: {}", file_path.display());
    let results = match ocr_app::process_docx(&state.engine, file_path) {
        Ok(r) => r,
        Err(e) => {
            println!("[DEBUG] DOCX processing error: {}", e);
            return Err(format!("Failed to process DOCX: {}", e));
        }
    };

    // Extract paragraphs and format as HTML
    println!("[DEBUG] Converting DOCX content to HTML");
    let mut html_content = String::from("<div class='docx-content'>");

    for paragraph in &results.paragraphs {
        html_content.push_str("<p>");
        html_content.push_str(&html_escape::encode_text(&paragraph));
        html_content.push_str("</p>");
    }
    html_content.push_str("</div>");

    // Return the results
    Ok(Json(DocxProcessResponse { 
        matches: results.full_matches,
        numbers: results.numbers,
        html_content,
        file_hash: hash
    }))
}

async fn process_pdf(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<ProcessResponse>, String> {
    println!("[DEBUG] Starting PDF processing");
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

    // Calculate SHA-256 hash
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = format!("{:x}", hasher.finalize());

    // Create a temporary file
    let temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temporary file: {}", e))?;

    // Write the data to the temporary file
    std::io::Write::write_all(&mut temp_file.as_file(), &data)
        .map_err(|e| format!("Failed to write to temporary file: {}", e))?;

    let file_path = temp_file.path();

    // Process the PDF
    println!("[DEBUG] Processing PDF file: {}", file_path.display());
    let results = match ocr_app::process_pdf(&state.engine, file_path) {
        Ok(r) => r,
        Err(e) => {
            println!("[DEBUG] PDF processing error: {}", e);
            return Err(format!("Failed to process PDF: {}", e));
        }
    };

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
    Ok(Json(ProcessResponse { 
        pages,
        file_hash: hash
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize OCR engine with models
    let detection_model_path = PathBuf::from("models/text-detection-checkpoint-03.23.recall_92.precis_85.rten");
    let rec_model_path = PathBuf::from("models/text-rec-checkpoint-7.rten");

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
    println!("[DEBUG] Setting up router");
    let app = Router::new()
        .route("/", get(index))
        .route("/process-pdf", post(process_pdf))
        .route("/process-docx", post(process_docx))
        .route("/comparison", get(comparison_view))
        .nest_service("/static", ServeDir::new("static"))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))  // 50MB limit
        .with_state(state);
    println!("[DEBUG] Router configured with routes: /, /process-pdf, /process-docx, /static");

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string()).parse::<u16>().unwrap();
    println!("Server running on port {}", port);
    let addr = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), port);
    println!("[DEBUG] Templates directory: {}", file_path("templates").display());
    println!("[DEBUG] Current working directory: {}", std::env::current_dir()?.display());
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("[DEBUG] Server bound to {}", addr);
    println!("[DEBUG] Starting server...");

    axum::serve(listener, app).await
        .context("Server error")?;

    Ok(())
}
