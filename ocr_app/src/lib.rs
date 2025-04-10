use std::path::Path;
use std::collections::HashSet;
use anyhow::{Context, Result};
use image::{ImageBuffer, Rgb, RgbImage};
use mupdf::{Colorspace, Device, Document, Matrix, Pixmap};
use ocrs::{ImageSource, OcrEngine};
use regex::Regex;
use docx_rs;

pub mod models;

/// Convert a PDF page to an RGB image
pub fn pdf_page_to_image(doc: &Document, page_num: i32, dpi: f32) -> Result<RgbImage> {
    let page = doc.load_page(page_num)
        .context("Failed to load PDF page")?;
    
    // Calculate dimensions based on DPI
    let bounds = page.bounds()
        .context("Failed to get page bounds")?;
    let scale = dpi / 72.0; // Convert from PDF points (72 DPI) to target DPI
    let width = ((bounds.x1 - bounds.x0) * scale) as i32;
    let height = ((bounds.y1 - bounds.y0) * scale) as i32;

    // Create transformation matrix for the desired scale
    let transform = Matrix::new_scale(scale, scale);

    // Create a pixmap with the desired dimensions
    let mut pixmap = Pixmap::new_with_w_h(
        &Colorspace::device_gray(),  // Use grayscale colorspace directly
        width,
        height,
        false // Disable alpha channel
    ).context("Failed to create pixmap")?;

    // Fill background with white
    pixmap.clear()
        .context("Failed to set background color")?;

    // Create a device for rendering
    let device = Device::from_pixmap(&pixmap)
        .context("Failed to create device")?;

    // Draw the page onto the pixmap
    page.run(&device, &transform)
        .context("Failed to render page to pixmap")?;

    // Convert pixmap to image::RgbImage
    let samples = pixmap.samples();
    
    let mut img = ImageBuffer::new(width as u32, height as u32);
    
    // Convert grayscale to black and white with a fixed threshold
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if idx < samples.len() {
                let gray = samples[idx];
                // Use a lower threshold to catch lighter text
                let value = if gray < 160 { 0 } else { 255 };
                let pixel = Rgb([value, value, value]);
                img.put_pixel(x as u32, y as u32, pixel);
            }
        }
    }

    Ok(img)
}

#[derive(serde::Serialize)]
pub struct OcrResult {
    pub text: String,
    pub bbox: [f32; 4],  // [x1, y1, x2, y2]
}

#[derive(serde::Serialize)]
pub struct DocxResult {
    pub full_matches: Vec<String>,  // Full matches like "word 123"
    pub numbers: Vec<String>,      // Just the numbers for comparison
    pub paragraphs: Vec<String>,   // Text content split into paragraphs
}



/// Process a single page and return the extracted text with bounding boxes
pub fn process_page(engine: &OcrEngine, mut img: RgbImage) -> Result<Vec<OcrResult>> {
    // Preprocess image to improve OCR
    for pixel in img.pixels_mut() {
        // Increase contrast
        let r = (pixel[0] as f32 * 1.2).min(255.0) as u8;
        let g = (pixel[1] as f32 * 1.2).min(255.0) as u8;
        let b = (pixel[2] as f32 * 1.2).min(255.0) as u8;
        *pixel = Rgb([r, g, b]);
    }

    // Convert image to OCR input format
    let img_source = ImageSource::from_bytes(img.as_raw(), img.dimensions())
        .map_err(|e| anyhow::anyhow!("Failed to create image source: {}", e))?;
    let ocr_input = engine.prepare_input(img_source)
        .map_err(|e| anyhow::anyhow!("Failed to prepare OCR input: {}", e))?;

    // Detect words and group into lines
    let word_rects = engine.detect_words(&ocr_input)
        .map_err(|e| anyhow::anyhow!("Failed to detect words: {}", e))?;
    let line_rects = engine.find_text_lines(&ocr_input, &word_rects);

    // Recognize text in each line
    let line_texts = engine.recognize_text(&ocr_input, &line_rects)
        .map_err(|e| anyhow::anyhow!("Failed to recognize text: {}", e))?;

    // Convert results to our format with bounding boxes
    let mut ocr_results = Vec::new();
    let (width, height) = img.dimensions();
    for (rects, texts) in line_rects.iter().zip(line_texts.iter()) {
        if let Some(text) = texts {
            // Get the bounding box that encompasses all rectangles in the line
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;

            for rect in rects {
                let corners = rect.corners();
                for corner in corners {
                    min_x = min_x.min(corner.x);
                    min_y = min_y.min(corner.y);
                    max_x = max_x.max(corner.x);
                    max_y = max_y.max(corner.y);
                }
            }

            ocr_results.push(OcrResult {
                text: text.to_string(),
                bbox: [
                    min_x / width as f32,   // Normalize coordinates
                    min_y / height as f32,
                    max_x / width as f32,
                    max_y / height as f32,
                ],
            });
        }
    }
    Ok(ocr_results)
}

pub fn process_docx(_engine: &OcrEngine, docx_path: impl AsRef<Path>) -> Result<DocxResult> {
    // Read DOCX file
    let docx_content = std::fs::read(docx_path)
        .context("Failed to read DOCX file")?;
    let docx = docx_rs::read_docx(&docx_content)
        .context("Failed to parse DOCX file")?;

    // Extract text and paragraphs from the document
    let mut text = String::new();
    let mut paragraphs = Vec::new();
    let mut current_paragraph = String::new();

    for child in docx.document.children {
        if let docx_rs::DocumentChild::Paragraph(para) = child {
            for child in para.children {
                if let docx_rs::ParagraphChild::Run(run) = child {
                    for child in run.children {
                        if let docx_rs::RunChild::Text(text_content) = child {
                            text.push_str(&text_content.text);
                            text.push(' ');
                            current_paragraph.push_str(&text_content.text);
                            current_paragraph.push(' ');
                        }
                    }
                }
            }
            if !current_paragraph.trim().is_empty() {
                paragraphs.push(current_paragraph.trim().to_string());
                current_paragraph.clear();
            }
            text.push('\n');
        }
    }

    // Create regex pattern for words followed by numbers
    let pattern = Regex::new(r"\b\w+\s+([0-9]+(?:-[0-9]+)?[a-zA-Z]?)\b")
        .context("Failed to create regex pattern")?;

    // Extract full matches and their numbers
    let mut full_matches_set = HashSet::new();
    let mut numbers = HashSet::new();
    let mut full_matches = Vec::new();

    for cap in pattern.captures_iter(&text) {
        // Get the full match (e.g., "word 123")
        let full_match = cap.get(0).unwrap().as_str().to_string();
        // Get just the number part (e.g., "123")
        let number = cap.get(1).unwrap().as_str().to_string();
        
        // Only add if we haven't seen this match before
        if full_matches_set.insert(full_match.clone()) {
            full_matches.push(full_match);
            numbers.insert(number);
        }
    }

    // Sort the full matches alphabetically by the word before the number
    full_matches.sort_by(|a, b| {
        // Extract the word part (everything before the number)
        let a_word = a.split_whitespace().next().unwrap_or("").to_lowercase();
        let b_word = b.split_whitespace().next().unwrap_or("").to_lowercase();

        // First compare words
        match a_word.cmp(&b_word) {
            std::cmp::Ordering::Equal => {
                // If words are the same, compare numbers
                let a_num = pattern.captures(a)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str())
                    .unwrap_or("");
                let b_num = pattern.captures(b)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str())
                    .unwrap_or("");

                let a_val = a_num.chars().take_while(|c| c.is_digit(10)).collect::<String>();
                let b_val = b_num.chars().take_while(|c| c.is_digit(10)).collect::<String>();

                match (a_val.parse::<i32>(), b_val.parse::<i32>()) {
                    (Ok(a_val), Ok(b_val)) => a_val.cmp(&b_val),
                    _ => a_num.cmp(b_num)
                }
            },
            other => other
        }
    });

    // Convert numbers set to sorted vector
    let mut numbers_vec: Vec<String> = numbers.into_iter().collect();
    numbers_vec.sort_by(|a, b| {
        let a_val = a.chars().take_while(|c| c.is_digit(10)).collect::<String>();
        let b_val = b.chars().take_while(|c| c.is_digit(10)).collect::<String>();

        match (a_val.parse::<i32>(), b_val.parse::<i32>()) {
            (Ok(a_val), Ok(b_val)) => a_val.cmp(&b_val),
            _ => a.cmp(b)
        }
    });

    Ok(DocxResult {
        full_matches,
        numbers: numbers_vec,
        paragraphs
    })
}

pub fn process_pdf(engine: &OcrEngine, pdf_path: impl AsRef<Path>) -> Result<Vec<(RgbImage, Vec<OcrResult>)>> {
    // Open PDF document
    let doc = Document::open(pdf_path.as_ref().to_str().unwrap())
        .context("Failed to open PDF file")?;

    // Get number of pages
    let page_count = doc.page_count()
        .context("Failed to get page count")?;

    let mut results = Vec::new();

    // Process each page
    for page_num in 0..page_count {
        // Convert PDF page to image (300 DPI for optimal quality)
        let img = pdf_page_to_image(&doc, page_num, 300.0)
            .context(format!("Failed to convert page {} to image", page_num + 1))?;

        // Process the page and extract text with bounding boxes
        let ocr_results = process_page(engine, img.clone())
            .context(format!("Failed to process page {}", page_num + 1))?;

        results.push((img, ocr_results));
    }

    Ok(results)
}
