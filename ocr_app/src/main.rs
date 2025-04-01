use std::collections::VecDeque;
use std::path::PathBuf;

use anyhow::{Context, Result};
use ocrs::{OcrEngine, OcrEngineParams};

struct Args {
    pdf_path: String,
}

fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut values = VecDeque::new();
    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        match arg {
            Value(val) => values.push_back(val.string()?),
            Long("help") => {
                println!(
                    "Usage: {bin_name} <pdf_file>",
                    bin_name = parser.bin_name().unwrap_or("ocr_app")
                );
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    let pdf_path = values.pop_front().ok_or("missing PDF file path")?;

    Ok(Args { pdf_path })
}

/// Given a file path relative to the crate root, return the absolute path.
fn file_path(path: &str) -> PathBuf {
    let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    abs_path.push(path);
    abs_path
}

fn main() -> Result<()> {
    let args = parse_args()?;

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
        debug: true, // Enable debug logging
        decode_method: ocrs::DecodeMethod::BeamSearch { width: 5 }, // Use beam search decoding
        ..Default::default()
    }).map_err(|e| anyhow::anyhow!("Failed to initialize OCR engine: {}", e))?;

    // Process PDF and get text from all pages
    let texts = ocr_app::process_pdf(&engine, &args.pdf_path)
        .context("Failed to process PDF")?;

    // Print extracted text
    for (i, text) in texts.iter().enumerate() {
        println!("Text from page {}:", i + 1);
        for line in text {
            println!("{}", line);
        }
        println!();
    }

    Ok(())
}
