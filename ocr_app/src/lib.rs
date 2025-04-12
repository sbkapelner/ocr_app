extern crate lazy_static;

use std::path::Path;
use std::collections::HashSet;
use anyhow::{Context, Result};
use image::{ImageBuffer, Rgb, RgbImage};
use mupdf::{Colorspace, Device, Document, Matrix, Pixmap};
use ocrs::{ImageSource, OcrEngine};
use regex::Regex;
use docx_rs;

pub mod models;

// Regex pattern for matching FIG/Figure references
static FIG_PATTERN: &str = r"(?i)\b(FIG\.?|FIGURE\.?|FIG|FIGURE)\s*([0-9]+)\s*([A-Za-z])?\b";

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
    
    // Process each line and its words
    for (line_rects, line_text) in line_rects.iter().zip(line_texts.iter()) {
        if let Some(text) = line_text {
            // First normalize the full line text for pattern matching
            let line_text = text.to_string();
            println!("[DEBUG] Raw OCR text: {}", line_text);
            let normalized_line = normalize_text(&line_text);
            
            // Process each word in the line
            for rect in line_rects {
                // Get the bounding box for this word
                let corners = rect.corners();
                let mut min_x = f32::MAX;
                let mut min_y = f32::MAX;
                let mut max_x = f32::MIN;
                let mut max_y = f32::MIN;
                
                for corner in corners {
                    min_x = min_x.min(corner.x);
                    min_y = min_y.min(corner.y);
                    max_x = max_x.max(corner.x);
                    max_y = max_y.max(corner.y);
                }

                // Use the normalized line text for pattern matching
                if normalized_line.chars().any(|c| c.is_ascii_digit()) {
                    // First look for FIG patterns in the line
                    let fig_regex = Regex::new(FIG_PATTERN).unwrap();
                    // Also look for standalone number-letter combinations that might be figure references
                    let standalone_ref_regex = Regex::new(r"\b([0-9]+[A-Z])\b").unwrap();
                    let mut results = Vec::new();

                    // Handle explicit FIG patterns first
                    for cap in fig_regex.captures_iter(&normalized_line) {
                        if let Some(number) = cap.get(2) {
                            let num_part = number.as_str();
                            if num_part.to_uppercase().contains(|c: char| c >= 'A' && c <= 'Z') {
                                // Get the original FIG prefix from the capture
                                let fig_prefix = cap.get(1).map(|m| m.as_str()).unwrap_or("FIG.");
                                results.push(format!("{}{}", fig_prefix, num_part));
                            }
                        }
                    }

                    // If no explicit FIG patterns found, look for standalone number-letter combinations
                    if results.is_empty() {
                        for cap in standalone_ref_regex.captures_iter(&normalized_line) {
                            if let Some(number) = cap.get(1) {
                                let num_part = number.as_str().to_uppercase();
                                results.push(format!("FIG.{}", num_part));
                            }
                        }
                    }

                    // If we found FIG patterns, use those
                    if !results.is_empty() {
                        // Remove duplicates while preserving order
                        let mut seen = HashSet::new();
                        results.retain(|x| seen.insert(x.clone()));
                        println!("[DEBUG] PDF numbers found: {:?}", results);
                        results.join(" ")
                    } else {
                        // Use default label options for OCR processing
                        let label_regex = build_label_regex(true, true, true, true, true);
                        let tokens: Vec<&str> = normalized_line.split_whitespace().collect();
                        
                        for raw_token in tokens {
                            let cleaned = clean_token(raw_token);
                            if cleaned.is_empty() { continue; }
                            
                            if label_regex.is_match(&cleaned) {
                                results.push(cleaned);
                            } else {
                                // Try to split merged tokens
                                let split_parts = split_merged_label(&cleaned, &label_regex);
                                results.extend(split_parts);
                            }
                        }
                        
                        // Remove duplicates while preserving order
                        let mut seen = HashSet::new();
                        results.retain(|x| seen.insert(x.clone()));
                        println!("[DEBUG] PDF numbers found: {:?}", results);
                        results.join(" ")
                    }
                } else {
                    normalized_line.clone()
                };
                
                // Log the exact text and its bounding box for debugging
                println!("[DEBUG] Bounding box text: '{}' at coordinates: [{:.3}, {:.3}, {:.3}, {:.3}]", 
                    text.to_string().trim(),
                    min_x / width as f32,
                    min_y / height as f32,
                    max_x / width as f32,
                    max_y / height as f32
                );
                
                // Only create result if we found patterns
                if !normalized_line.is_empty() {
                    ocr_results.push(OcrResult {
                        text: normalized_line.clone(),
                        bbox: [
                            min_x / width as f32,   // Normalize coordinates
                            min_y / height as f32,
                            max_x / width as f32,
                            max_y / height as f32,
                        ],
                    });
                }
            }
        }
    }
    Ok(ocr_results)
}

/// Helper function to normalize text by converting plurals to singular form
fn normalize_text(text: &str) -> String {
    // Check for FIG references first and preserve them
    let fig_regex = Regex::new(FIG_PATTERN).unwrap();
    if let Some(cap) = fig_regex.captures(text) {
        if let Some(number) = cap.get(2) {
            let num_part = number.as_str();
            if num_part.to_uppercase().contains(|c: char| c >= 'A' && c <= 'Z') {
                return format!("FIG.{}", num_part);
            }
        }
    }
    
    // If not a FIG reference, convert to lowercase
    let text = text.to_lowercase();
    
    // Split into words
    let words: Vec<&str> = text.split_whitespace().collect();
    let normalized_words: Vec<String> = words.iter()
        .map(|word| {
            let w = *word;
            if w.len() < 3 { return w.to_string(); }  // Too short to be plural
            
            // Rule 1: words ending in 'ies' -> 'y'
            if w.ends_with("ies") && w.len() > 3 {
                return format!("{}{}", &w[..w.len()-3], "y");
            }
            
            // Rule 2: words ending in 'es' -> remove 'es'
            if w.ends_with("es") {
                // Special case: if word ends in 'xes', 'ches', 'shes', 'sses'
                if w.ends_with("xes") || w.ends_with("ches") || 
                   w.ends_with("shes") || w.ends_with("sses") {
                    return w[..w.len()-2].to_string();
                }
                return w[..w.len()-2].to_string();
            }
            
            // Rule 3: words ending in 's' -> remove 's'
            if w.ends_with('s') && !w.ends_with("ss") {
                return w[..w.len()-1].to_string();
            }
            
            w.to_string()
        })
        .collect();
    
    let result = normalized_words.join(" ");
    // Convert 'fig' back to 'FIG'
    // Convert all variants of 'fig' to 'FIG'
    result.replace("fig.", "FIG.")
         .replace("fig ", "FIG ")
         .replace("fig
", "FIG
")
}

/// Build a regex pattern for matching valid label numbers
fn build_label_regex(allow_2: bool, allow_3: bool, allow_4: bool, allow_letters: bool, allow_hyphen: bool) -> Regex {
    let mut patterns = Vec::new();
    
    if allow_2 {
        patterns.push(r"\d{2}");
        if allow_letters {
            patterns.push(r"\d{2}[a-zA-Z]");
        }
    }
    if allow_3 {
        patterns.push(r"\d{3}");
        if allow_letters {
            patterns.push(r"\d{3}[a-zA-Z]");
        }
        if allow_hyphen {
            patterns.push(r"\d{3}-\d");
        }
    }
    if allow_4 {
        patterns.push(r"\d{4}");
        if allow_letters {
            patterns.push(r"\d{4}[a-zA-Z]");
        }
        if allow_hyphen {
            patterns.push(r"\d{4}-\d");
        }
    }
    
    Regex::new(&format!(r"({})", patterns.join("|"))).unwrap()
}

/// Clean a token by removing all non-word and non-hyphen characters
fn clean_token(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

/// Try to split a merged label into valid parts
fn split_merged_label(token: &str, label_regex: &Regex) -> Vec<String> {
    let token = clean_token(token);
    let mut results = Vec::new();
    
    // Try all possible splits
    for i in 2..token.len()-1 {
        let left = &token[..i];
        let right = &token[i..];
        
        if label_regex.is_match(left) && label_regex.is_match(right) {
            results.push(left.to_string());
            results.push(right.to_string());
            break;  // assume only one valid split
        }
    }
    
    results
}

/// Helper function to normalize a number by applying label rules
pub fn normalize_number(num: &str, allow_2: bool, allow_3: bool, allow_4: bool, allow_letters: bool, allow_hyphen: bool) -> String {
    // Initialize regex pattern with provided options
    let label_regex = build_label_regex(allow_2, allow_3, allow_4, allow_letters, allow_hyphen);
    
    // Special handling for FIG. variations
    let fig_regex = Regex::new(FIG_PATTERN).unwrap();
    if let Some(cap) = fig_regex.captures(num) {
        if let Some(number) = cap.get(2) {
            let number_str = number.as_str();
            if label_regex.is_match(number_str) {
                // Normalize to consistent "FIG. " format
                return format!("FIG. {}", number_str);
            }
        }
    }
    
    // Split input on whitespace
    let tokens: Vec<&str> = num.split_whitespace().collect();
    let mut results = Vec::new();
    
    for raw_token in tokens {
        let cleaned = clean_token(raw_token);
        if cleaned.is_empty() { continue; }
        
        if label_regex.is_match(&cleaned) {
            results.push(cleaned);
        } else {
            // Try to split merged tokens
            let split_parts = split_merged_label(&cleaned, &label_regex);
            results.extend(split_parts);
        }
    }
    
    // Remove duplicates while preserving order
    let mut seen = HashSet::new();
    results.retain(|x| seen.insert(x.clone()));
    
    results.join(" ")
}

pub fn process_docx(_engine: &OcrEngine, docx_path: impl AsRef<Path>, allow_2: bool, allow_3: bool, allow_4: bool, allow_letters: bool, allow_hyphen: bool) -> Result<DocxResult> {
    // Read DOCX file
    let docx_content = std::fs::read(docx_path)
        .context("Failed to read DOCX file")?;
    let docx = docx_rs::read_docx(&docx_content)
        .context("Failed to parse DOCX file")?;

    // Extract text and paragraphs from the document
    println!("[DEBUG] Starting DOCX text extraction");
    let mut text = String::new();
    let mut paragraphs = Vec::new();
    let mut current_paragraph = String::new();

    // Process each paragraph
    for child in docx.document.children {
        if let docx_rs::DocumentChild::Paragraph(para) = child {
            // Extract all text from the paragraph
            let para_text: String = para.children.iter()
                .filter_map(|child| {
                    if let docx_rs::ParagraphChild::Run(run) = child {
                        Some(run.children.iter().filter_map(|child| {
                            if let docx_rs::RunChild::Text(text) = child {
                                Some(text.text.as_str())
                            } else {
                                None
                            }
                        }).collect::<Vec<_>>().join(""))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            println!("[DEBUG] Paragraph text after joining: {}", para_text);
            text.push_str(&para_text);
            text.push('\n');
            current_paragraph.push_str(&para_text);
            if !current_paragraph.trim().is_empty() {
                paragraphs.push(current_paragraph.trim().to_string());
                current_paragraph.clear();
            }
            text.push('\n');
        }
    }

    println!("[DEBUG] Final collected text:\n{}", text);

    // Create regex patterns
    let word_pattern = Regex::new(r"\b(\w+)\s+([^\s]*[0-9][^\s]*)\b")
        .context("Failed to create word pattern")?;
    let fig_pattern = Regex::new(FIG_PATTERN)
        .context("Failed to create FIG pattern")?;

    // Extract full matches and their numbers
    let mut normalized_matches = HashSet::new();
    let mut numbers = HashSet::new();
    let mut full_matches = Vec::new();

    // Keep track of the last meaningful noun for "and NUMBER" cases
    let mut last_noun = String::new();

    // First process FIG patterns
    println!("[DEBUG] Processing text for FIG patterns:");
    for paragraph in text.split('\n') {
        println!("[DEBUG] Processing paragraph: {}", paragraph);
        for cap in fig_pattern.captures_iter(paragraph) {
            println!("[DEBUG] Found capture: {:?}", cap.iter().map(|m| m.map(|m| m.as_str())).collect::<Vec<_>>());
            let number = cap.get(2).unwrap().as_str().trim();
            let letter = cap.get(3).map(|m| m.as_str().trim());
            
            let fig_text = if let Some(letter) = letter {
                format!("FIG. {}{}", number, letter)
            } else {
                format!("FIG. {}", number)
            };
            
            println!("[DEBUG] Found FIG match: {}", fig_text);
            if normalized_matches.insert(fig_text.clone()) {
                println!("[DEBUG] Adding FIG match: {}", fig_text);
                full_matches.push(fig_text.clone());
                numbers.insert(fig_text);
            }
        }
    }
    
    for cap in word_pattern.captures_iter(&text) {
        let word = normalize_text(cap.get(1).unwrap().as_str().trim());
        let raw_number = cap.get(2).unwrap().as_str().trim();
        
        // Skip if the raw number doesn't contain any digits
        if !raw_number.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        
        // Skip unwanted prefixes and FIG references
        if word.eq_ignore_ascii_case("about") || word.eq_ignore_ascii_case("of") || word.eq_ignore_ascii_case("fig") || word.eq_ignore_ascii_case("figure") {
            continue;
        }
        
        let normalized_number = normalize_number(raw_number, allow_2, allow_3, allow_4, allow_letters, allow_hyphen);
        if normalized_number.is_empty() {
            continue;
        }
        
        let mut full_match = cap.get(0).unwrap().as_str().trim().to_string();
        
        // Handle "and NUMBER" and "or NUMBER" cases
        let is_conjunction = word.eq_ignore_ascii_case("and") || word.eq_ignore_ascii_case("or");
        if is_conjunction && !last_noun.is_empty() {
            full_match = format!("{} {}", last_noun, raw_number);
        } else if !is_conjunction {
            // Update last noun for next iteration
            last_noun = word.clone();
        }
        
        // Check if we've seen a similar word with the same number
        let current_word = if is_conjunction { &last_noun } else { &word };
        let mut found_similar = false;
        
        // Compare with existing matches
        for existing_match in &full_matches {
            if let Some(existing_word) = existing_match.split_whitespace().next() {
                // If the words are similar (one is a prefix of the other) and have the same number
                if (existing_word.starts_with(current_word) || current_word.starts_with(existing_word)) 
                   && existing_match.split_whitespace().nth(1) == Some(raw_number) {
                    found_similar = true;
                    break;
                }
            }
        }
        
        // Only add if we haven't seen a similar match before
        if !found_similar {
            full_matches.push(full_match);
            numbers.insert(normalized_number);
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
                let a_num = word_pattern.captures(a)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str())
                    .unwrap_or("");
                let b_num = word_pattern.captures(b)
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

    println!("[DEBUG] DOCX full_matches before sort: {:?}", full_matches);
    println!("[DEBUG] DOCX numbers before sort: {:?}", numbers);

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

    println!("[DEBUG] DOCX full_matches: {:?}", full_matches);
    println!("[DEBUG] DOCX numbers: {:?}", numbers_vec);

    println!("[DEBUG] DOCX final full_matches after sort: {:?}", full_matches);
    println!("[DEBUG] DOCX final numbers after sort: {:?}", numbers_vec);

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
