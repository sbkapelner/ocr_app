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

    // Process PDF and get text and images from all pages
    let results = ocr_app::process_pdf(&engine, &args.pdf_path)
        .context("Failed to process PDF")?;

    // Print extracted text and save images
    for (i, (image, ocr_results)) in results.iter().enumerate() {
        println!("Text from page {}:", i + 1);
        for result in ocr_results {
            println!("{}", result.text);
        }
        println!();

        // Save the image with bounding boxes
        let mut output_image = image.clone();
        for result in ocr_results {
            // Convert normalized coordinates back to pixel coordinates
            let [x1, y1, x2, y2] = result.bbox;
            let width = output_image.width() as f32;
            let height = output_image.height() as f32;
            let x1 = (x1 * width) as u32;
            let y1 = (y1 * height) as u32;
            let x2 = (x2 * width) as u32;
            let y2 = (y2 * height) as u32;

            // Draw red rectangle
            for x in x1..=x2 {
                if x < output_image.width() {
                    if y1 < output_image.height() {
                        output_image.put_pixel(x, y1, image::Rgb([255, 0, 0]));
                    }
                    if y2 < output_image.height() {
                        output_image.put_pixel(x, y2, image::Rgb([255, 0, 0]));
                    }
                }
            }
            for y in y1..=y2 {
                if y < output_image.height() {
                    if x1 < output_image.width() {
                        output_image.put_pixel(x1, y, image::Rgb([255, 0, 0]));
                    }
                    if x2 < output_image.width() {
                        output_image.put_pixel(x2, y, image::Rgb([255, 0, 0]));
                    }
                }
            }
        }

        // Save the image
        output_image.save(format!("output_page_{}.png", i + 1))
            .context(format!("Failed to save output image for page {}", i + 1))?;
    }

    Ok(())
}
