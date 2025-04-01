use std::path::Path;
use anyhow::{Context, Result};
use image::{ImageBuffer, Rgb, RgbImage};
use mupdf::{Colorspace, Device, Document, Matrix, Pixmap};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};

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

/// Process a single page and return the extracted text
pub fn process_page(engine: &OcrEngine, mut img: RgbImage) -> Result<Vec<String>> {
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

    // Filter and collect text lines
    // Keep raw OCR output without filtering
    Ok(line_texts
        .iter()
        .flatten()
        .map(|l| l.to_string())
        .collect())
}

pub fn process_pdf(engine: &OcrEngine, pdf_path: impl AsRef<Path>) -> Result<Vec<Vec<String>>> {
    // Open PDF document
    let doc = Document::open(pdf_path.as_ref().to_str().unwrap())
        .context("Failed to open PDF file")?;

    // Get number of pages
    let page_count = doc.page_count()
        .context("Failed to get page count")?;

    let mut all_text = Vec::new();

    // Process each page
    for page_num in 0..page_count {
        // Convert PDF page to image (300 DPI for optimal quality)
        let img = pdf_page_to_image(&doc, page_num, 300.0)
            .context(format!("Failed to convert page {} to image", page_num + 1))?;

        // Process the page and extract text
        let text = process_page(engine, img)
            .context(format!("Failed to process page {}", page_num + 1))?;

        all_text.push(text);
    }

    Ok(all_text)
}
