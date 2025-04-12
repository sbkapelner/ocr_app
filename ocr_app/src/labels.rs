#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LabelFormat {
    ThreeDigit,      // 130
    TwoDigit,        // 13
    ThreeDigitLetter, // 130a
    TwoDigitLetter,  // 13a
    ThreeDigitDash,  // 130-1
}

/// Clean and normalize text from OCR
pub fn clean_text(text: &str) -> String {
    // Keep FIG. prefix if present
    if text.to_uppercase().contains("FIG") {
        return text.to_string();
    }

    // Otherwise clean up the text
    text.trim()
        .replace('l', "1")
        .replace('O', "0")
        .trim()
        .to_string()
}

/// Check if text matches a specific label format
pub fn matches_format(text: &str, format: LabelFormat) -> bool {
    let text = text.trim();
    
    // Special handling for FIG. prefix
    if text.to_uppercase().contains("FIG") {
        return true;
    }

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

/// Check if text is a valid label according to selected formats
pub fn is_valid_label(text: &str, allowed_formats: &[LabelFormat]) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_formats() {
        let formats = vec![
            LabelFormat::ThreeDigit,
            LabelFormat::TwoDigit,
            LabelFormat::ThreeDigitLetter,
            LabelFormat::TwoDigitLetter,
            LabelFormat::ThreeDigitDash,
        ];

        // Test valid cases
        assert!(is_valid_label("130", &formats).is_some());
        assert!(is_valid_label("13", &formats).is_some());
        assert!(is_valid_label("130a", &formats).is_some());
        assert!(is_valid_label("13a", &formats).is_some());
        assert!(is_valid_label("130-1", &formats).is_some());
        assert!(is_valid_label("FIG.6", &formats).is_some());
        assert!(is_valid_label("FIG. 1B", &formats).is_some());
        assert!(is_valid_label("130.", &formats).is_some());

        // Test invalid cases
        assert!(is_valid_label("W", &formats).is_none());
        assert!(is_valid_label("T", &formats).is_none());
        assert!(is_valid_label("15?", &formats).is_none());
        assert!(is_valid_label("1104", &formats).is_none());
        assert!(is_valid_label("130-1130-2", &formats).is_none());
    }
}
