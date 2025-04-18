use anyhow::Result;
use image::{ImageBuffer, Rgb};
use mupdf::{Document, Matrix, Pixmap, Colorspace, Device};

const SAMPLE_DRAW_PDF: &str = "test 6.pdf";
const DPI: f32 = 150.0;

fn pdf_page_to_image(doc: &Document, page_num: i32, dpi: f32) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    let page = doc.load_page(page_num)?;
    let scale = dpi / 72.0;
    let transform = Matrix::new_scale(scale, scale);
    let bounds = page.bounds()?;
    let width = ((bounds.x1 - bounds.x0) * scale) as i32;
    let height = ((bounds.y1 - bounds.y0) * scale) as i32;

    let mut pixmap = Pixmap::new_with_w_h(
        &Colorspace::device_gray(),
        width,
        height,
        false
    )?;
    pixmap.clear()?;

    let device = Device::from_pixmap(&pixmap)?;
    page.run(&device, &transform)?;

    let mut img = ImageBuffer::new(width as u32, height as u32);
    let samples = pixmap.samples();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if idx < samples.len() {
                let gray = samples[idx];
                let value = if gray < 160 { 0 } else { 255 };
                img.put_pixel(x as u32, y as u32, Rgb([value, value, value]));
            }
        }
    }

    Ok(img)
}

fn main() -> Result<()> {


    let doc = Document::open(SAMPLE_DRAW_PDF)?;
    let page_count = doc.page_count()?;

    println!("Found {} pages in PDF", page_count);
    
    for page_num in 0..page_count {
        println!("Processing page {}", page_num + 1);
        let img = pdf_page_to_image(&doc, page_num, DPI)?;
        
        let output_path = format!("page_{}.png", (page_num+60) + 1);
        img.save(&output_path)?;
        println!("Saved {}", output_path);
    }

    println!("All pages extracted successfully!");
    Ok(())
}
