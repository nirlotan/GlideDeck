//! PDF export: one page per slide on the path, each rendered by the same GPU
//! renderer as the show and embedded as a high-quality JPEG. The writer is a
//! few dozen lines of plain PDF 1.7; nothing is sent anywhere.

use glide_core::{Camera, Document};
use glide_render::CanvasRenderer;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

/// Page width in points (13.33 in, PowerPoint's 16:9 slide); height follows each slide's shape.
const PAGE_WIDTH_PT: f64 = 960.0;

#[derive(Clone, Copy, Debug)]
pub struct PdfOptions {
    /// Rendered width of each slide in pixels.
    pub width_px: u32,
    /// JPEG quality, 1–100.
    pub quality: u8,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions { width_px: 2560, quality: 90 }
    }
}

pub struct PdfReport {
    pub pages: usize,
    pub bytes: usize,
}

/// Renders every slide on the path and writes them to `out` as a PDF.
pub fn export(renderer: &mut CanvasRenderer, doc: &Document, out: &Path, opts: PdfOptions) -> Result<PdfReport, String> {
    if doc.path.is_empty() {
        return Err("the presentation has no slides".into());
    }
    let mut pages = Vec::new();
    for i in 0..doc.path.len() {
        let Some(frame) = doc.step_frame(i) else { continue };
        let size = frame.kind.intrinsic_size().unwrap_or(glam::DVec2::new(16.0, 9.0));
        let aspect = (size.x / size.y.max(1e-9)).clamp(0.1, 10.0);
        let w = opts.width_px.clamp(160, 8192);
        let h = ((w as f64 / aspect).round() as u32).max(1);
        let vp = glam::DVec2::new(w as f64, h as f64);
        let camera = Camera::for_frame(frame, vp, 0.0).ok_or_else(|| format!("slide {} has an empty frame", i + 1))?;
        let rgba = renderer.snapshot(doc, &camera, [w, h], false).map_err(|e| e.to_string())?;
        pages.push(Page { width: w, height: h, jpeg: jpeg(&rgba, w, h, opts.quality)? });
    }
    let bytes = write_pdf(&pages, &doc.title)?;
    std::fs::write(out, &bytes).map_err(|e| format!("writing {}: {e}", out.display()))?;
    Ok(PdfReport { pages: pages.len(), bytes: bytes.len() })
}

pub struct Page {
    pub width: u32,
    pub height: u32,
    pub jpeg: Vec<u8>,
}

/// Encodes RGBA pixels as JPEG, flattening any transparency onto white.
fn jpeg(rgba: &[u8], w: u32, h: u32, quality: u8) -> Result<Vec<u8>, String> {
    let rgb: Vec<u8> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = p[3] as u32;
            let over = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
            [over(p[0]), over(p[1]), over(p[2])]
        })
        .collect();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
        .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// A PDF text string: plain ASCII when possible, else UTF-16BE hex.
fn pdf_string(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
        let escaped = s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
        format!("({escaped})")
    } else {
        let mut hex = String::from("<FEFF");
        for unit in s.encode_utf16() {
            let _ = write!(hex, "{unit:04X}");
        }
        hex + ">"
    }
}

/// Writes image-only pages as a PDF document.
pub fn write_pdf(pages: &[Page], title: &str) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
    // Object numbers: 1 catalog, 2 page tree, 3 info, then 3 per page.
    let page_obj = |i: usize| 4 + i * 3;
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let n = offsets.len();
        let _ = writeln!(out, "{n} 0 obj");
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", page_obj(i))).collect();
    obj(&mut out, &mut offsets, format!("<< /Type /Pages /Count {} /Kids [{}] >>", pages.len(), kids.join(" ")).as_bytes());
    obj(&mut out, &mut offsets, format!("<< /Title {} /Producer (GlideDeck {}) >>", pdf_string(title), env!("CARGO_PKG_VERSION")).as_bytes());
    for (i, page) in pages.iter().enumerate() {
        let (pw, ph) = (PAGE_WIDTH_PT, PAGE_WIDTH_PT * page.height as f64 / page.width.max(1) as f64);
        let (p, c, im) = (page_obj(i), page_obj(i) + 1, page_obj(i) + 2);
        obj(
            &mut out,
            &mut offsets,
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im{i} {im} 0 R >> >> /Contents {c} 0 R >>").as_bytes(),
        );
        debug_assert_eq!(offsets.len(), p);
        let content = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /Im{i} Do Q");
        obj(&mut out, &mut offsets, format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes());
        let mut image = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            page.width,
            page.height,
            page.jpeg.len()
        )
        .into_bytes();
        image.extend_from_slice(&page.jpeg);
        image.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offsets, &image);
    }
    let xref = out.len();
    let _ = write!(out, "xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
    for o in &offsets {
        let _ = writeln!(out, "{o:010} 00000 n ");
    }
    let _ = write!(out, "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_well_formed_pdf() {
        let rgba = vec![200u8; 32 * 18 * 4];
        let page = Page { width: 32, height: 18, jpeg: jpeg(&rgba, 32, 18, 80).unwrap() };
        let pdf = write_pdf(&[page], "Quarterly — review").unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("/Count 1") && text.contains("/Filter /DCTDecode"));
        assert!(text.contains("/Title <FEFF"), "non-ASCII titles are UTF-16");
        assert!(text.trim_end().ends_with("%%EOF"));
        // Every xref offset points at its object.
        let xref_at: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        let table = &text[xref_at..];
        for (n, line) in table.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(text[off..].starts_with(&format!("{} 0 obj", n + 1)), "object {} at {off}", n + 1);
        }
        assert_eq!(pdf_string("a(b)"), "(a\\(b\\))");
    }
}
