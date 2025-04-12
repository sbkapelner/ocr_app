use ocrs::{OcrEngine, OcrEngineParams, DecodeMethod};
use anyhow::Result;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LabelFormat {
    ThreeDigit,      // 130
    TwoDigit,        // 13
    ThreeDigitLetter, // 130a
    TwoDigitLetter,  // 13a
    ThreeDigitDash,  // 130-1
}

fn clean_text(text: &str) -> String {
    // Keep FIG. prefix if present
    if text.to_uppercase().contains("FIG") {
        return text.to_string();
    }

    // Otherwise clean up the text
    let mut cleaned = text.trim().to_string();
    cleaned = cleaned.replace('l', "1");
    cleaned = cleaned.replace('O', "0");
    cleaned.trim().to_string()
}

fn matches_format(text: &str, format: LabelFormat) -> bool {
    let text = text.trim();
    
    // Special handling for FIG. prefix
    if text.to_uppercase().contains("FIG") {
        return true;
    }

    // Handle normal cases
    match format {
        LabelFormat::ThreeDigit => {
            text.len() == 3 && text.chars().all(|c| c.is_ascii_digit())
        },
        LabelFormat::TwoDigit => {
            text.len() == 2 && text.chars().all(|c| c.is_ascii_digit())
        },
        LabelFormat::ThreeDigitLetter => {
            text.len() == 4 
            && text[..3].chars().all(|c| c.is_ascii_digit())
            && text[3..].chars().all(|c| c.is_ascii_lowercase())
        },
        LabelFormat::TwoDigitLetter => {
            text.len() == 3
            && text[..2].chars().all(|c| c.is_ascii_digit())
            && text[2..].chars().all(|c| c.is_ascii_lowercase())
        },
        LabelFormat::ThreeDigitDash => {
            let parts: Vec<&str> = text.split('-').collect();
            parts.len() == 2 
            && parts[0].len() == 3 
            && parts[0].chars().all(|c| c.is_ascii_digit())
            && parts[1].chars().all(|c| c.is_ascii_digit())
            && !parts[1].is_empty()
        },
    }
}

fn is_valid_label(text: &str, allowed_formats: &HashSet<LabelFormat>) -> Option<String> {
    // Early rejects
    if text.trim().is_empty() {
        return None;
    }

    let cleaned = clean_text(text);
    
    // Keep if it matches any allowed format
    if allowed_formats.iter().any(|&format| matches_format(&cleaned, format)) {
        Some(cleaned)
    } else {
        None
    }
}

const SAMPLE_DRAW_PDF: &str = "sample_draw.pdf";
const SAMPLE_SPEC_DOCX: &str = "sample_spec";

fn process_pdf(allowed_formats: &HashSet<LabelFormat>) -> Result<()> {
    // Load models
    let detection_model = ocr_app::models::load_model("models/text-detection-checkpoint-03.23.recall_92.precis_85.rten")?;
    let recognition_model = ocr_app::models::load_model("models/text-rec-checkpoint-7.rten")?;

    let params = OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        debug: false,
        decode_method: DecodeMethod::BeamSearch { width: 5 },
        ..Default::default()
    };
    let engine = OcrEngine::new(params)?;
    
    // Process the drawing PDF
    println!("\n=== Testing PDF Processing ===");
    println!("Processing: {}", SAMPLE_DRAW_PDF);
    let results = ocr_app::process_pdf(&engine, SAMPLE_DRAW_PDF)?;
    
    // Print results for each page
    for (page_num, (_, ocr_results)) in results.iter().enumerate() {
        println!("\nPage {}", page_num + 1);
        println!("Found {} text regions", ocr_results.len());
        
        for result in ocr_results {
            if let Some(valid_label) = is_valid_label(&result.text, allowed_formats) {
                println!("KEEP: '{}' (original: '{}')", valid_label, result.text);
            } else {
                println!("THROW: '{}'", result.text);
            }
        }
    }
    
    Ok(())
}

fn process_docx() -> Result<()> {
        let params = OcrEngineParams::default();
        let engine = OcrEngine::new(params)?;
        
        // Process the spec document
        println!("\n=== Testing DOCX Processing ===");
        println!("Processing: {}", SAMPLE_SPEC_DOCX);
        let result = ocr_app::process_docx(
            &engine,
            SAMPLE_SPEC_DOCX,
            true,  // allow_2
            true,  // allow_3
            true,  // allow_4
            true,  // allow_letters
            true,  // allow_hyphen
        )?;
        
        println!("\nFull matches:");
        for m in &result.full_matches {
            println!("  {}", m);
        }
        
        println!("\nExtracted numbers:");
        for n in &result.numbers {
            println!("  {}", n);
        }
        
        println!("\nParagraphs:");
        for (i, p) in result.paragraphs.iter().enumerate() {
            println!("\n[{}] {}", i + 1, p);
        }
        
        Ok(())
    }

fn main() -> Result<()> {
    // Test with different format combinations
    let test_formats = vec![
        (vec![LabelFormat::ThreeDigit, LabelFormat::TwoDigit], "3-digit and 2-digit"),
        (vec![LabelFormat::ThreeDigitLetter, LabelFormat::TwoDigitLetter], "3-digit+letter and 2-digit+letter"),
        (vec![LabelFormat::ThreeDigit, LabelFormat::ThreeDigitDash], "3-digit and 3-digit-dash"),
    ];

    for (formats, desc) in test_formats {
        println!("\nTesting with formats: {}", desc);
        let format_set: HashSet<LabelFormat> = formats.into_iter().collect();
        process_pdf(&format_set)?;
    }

    Ok(())
}
