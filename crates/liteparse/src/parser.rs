use crate::config::{LiteParseConfig, parse_target_pages};
#[cfg(not(target_arch = "wasm32"))]
use crate::conversion;
#[cfg(not(target_arch = "wasm32"))]
use crate::conversion::convert_data_to_pdf;
use crate::error::LiteParseError;
use crate::extract;
use crate::ocr::OcrEngine;
#[cfg(not(target_arch = "wasm32"))]
use crate::ocr::http_simple::HttpOcrEngine;
#[cfg(feature = "tesseract")]
use crate::ocr::tesseract::TesseractOcrEngine;
use crate::ocr_merge;
use crate::projection;
use crate::types::{ParsedPage, PdfInput};

/// Result of parsing a document.
pub struct ParseResult {
    /// Parsed pages with projected text layout.
    pub pages: Vec<ParsedPage>,
    /// Full document text, concatenated from all pages.
    pub text: String,
}

/// Main LiteParse orchestrator.
pub struct LiteParse {
    config: LiteParseConfig,
    /// Optional caller-provided OCR engine. When set, this overrides the
    /// built-in selection logic (HTTP OCR / Tesseract). This is the primary
    /// mechanism for plugging an OCR engine in environments without the
    /// built-ins (e.g. WASM, where the JS side supplies a callback engine).
    ocr_engine_override: Option<std::sync::Arc<dyn OcrEngine>>,
}

impl LiteParse {
    pub fn new(config: LiteParseConfig) -> Self {
        Self {
            config,
            ocr_engine_override: None,
        }
    }

    /// Override the OCR engine. When set, the engine is used regardless of
    /// `ocr_server_url` / built-in Tesseract availability.
    pub fn with_ocr_engine(mut self, engine: std::sync::Arc<dyn OcrEngine>) -> Self {
        self.ocr_engine_override = Some(engine);
        self
    }

    /// Parse a document from a file path, returning structured results.
    ///
    /// Non-PDF files are automatically converted to PDF first (requires
    /// LibreOffice/ImageMagick on the system).
    ///
    /// Not available on `wasm32` — the browser has no filesystem. Use
    /// [`LiteParse::parse_input`] with [`PdfInput::Bytes`] instead.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn parse(&self, input: &str) -> Result<ParseResult, LiteParseError> {
        self.parse_input(PdfInput::Path(input.to_string())).await
    }

    /// Parse a document from either a file path or raw bytes.
    ///
    /// Use `PdfInput::Path` for files on disk or `PdfInput::Bytes` for
    /// in-memory PDF data (e.g. from a network response or Node.js Buffer).
    pub async fn parse_input(&self, input: PdfInput) -> Result<ParseResult, LiteParseError> {
        let log = |msg: &str| {
            if !self.config.quiet {
                eprintln!("{}", msg);
            }
        };

        let t0 = web_time::Instant::now();

        #[cfg(not(target_arch = "wasm32"))]
        let mut _buffer_temps: Vec<tempfile::TempDir> = Vec::new();
        #[cfg(not(target_arch = "wasm32"))]
        let mut _conv_tmp_dir: Option<tempfile::TempDir> = None;

        let validated_input = {
            #[cfg(target_arch = "wasm32")]
            {
                input
            }

            #[cfg(not(target_arch = "wasm32"))]
            match input {
                PdfInput::Path(p) => {
                    if conversion::is_pdf(&p) {
                        PdfInput::Path(p)
                    } else {
                        let (converted, tmp_dir) =
                            conversion::convert_to_pdf(&p, self.config.password.as_deref()).await?;
                        _conv_tmp_dir = tmp_dir;
                        // The on-disk source isn't ours to delete; only the
                        // converted temp file should be cleaned up by `tmp_dir`.
                        PdfInput::Path(converted.pdf_path)
                    }
                }
                PdfInput::Bytes(b) => {
                    let (converted, temps) =
                        convert_data_to_pdf(b, self.config.password.as_deref()).await?;
                    _buffer_temps = temps;
                    PdfInput::Path(converted.pdf_path)
                }
            }
        };

        // Determine which pages to extract
        let target_pages = self
            .config
            .target_pages
            .as_ref()
            .map(|s| parse_target_pages(s))
            .transpose()
            .map_err(|e| format!("invalid --target-pages: {}", e))?;

        // Extract text items from PDF pages
        let mut pages = extract::extract_pages_from_input(
            &validated_input,
            target_pages.as_deref(),
            self.config.max_pages,
            self.config.password.as_deref(),
        )?;
        let t1 = web_time::Instant::now();
        log(&format!(
            "[liteparse] extract: {:.1}ms ({} pages)",
            t1.duration_since(t0).as_secs_f64() * 1000.0,
            pages.len()
        ));

        // OCR pass
        if self.config.ocr_enabled {
            let engine: std::sync::Arc<dyn OcrEngine> = if let Some(e) =
                self.ocr_engine_override.clone()
            {
                e
            } else {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(ref url) = self.config.ocr_server_url {
                        std::sync::Arc::new(HttpOcrEngine::new(url.clone()))
                    } else {
                        #[cfg(feature = "tesseract")]
                        {
                            std::sync::Arc::new(TesseractOcrEngine::new(
                                self.config.tessdata_path.clone(),
                            ))
                        }
                        #[cfg(not(feature = "tesseract"))]
                        {
                            return Err("OCR enabled but no --ocr-server-url provided and tesseract feature is disabled".into());
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    return Err(
                        "OCR enabled but no `ocrEngine` callback was provided (WASM builds have no built-in OCR engine)".into(),
                    );
                }
            };
            ocr_merge::ocr_and_merge_pages_from_input(
                &mut pages,
                &validated_input,
                self.config.dpi,
                engine,
                &self.config.ocr_language,
                self.config.num_workers,
            )
            .await?;
        }
        let t_ocr = web_time::Instant::now();
        log(&format!(
            "[liteparse] ocr: {:.1}ms",
            t_ocr.duration_since(t1).as_secs_f64() * 1000.0
        ));

        // Grid projection
        let parsed_pages = projection::project_pages_to_grid(pages);
        let t2 = web_time::Instant::now();
        log(&format!(
            "[liteparse] project: {:.1}ms",
            t2.duration_since(t_ocr).as_secs_f64() * 1000.0
        ));

        let full_text = parsed_pages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let total = t2.duration_since(t0).as_secs_f64() * 1000.0;
        log(&format!("[liteparse] total: {:.1}ms", total));

        Ok(ParseResult {
            pages: parsed_pages,
            text: full_text,
        })
    }

    pub fn config(&self) -> &LiteParseConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_new_stores_config() {
        let mut cfg = LiteParseConfig::default();
        cfg.ocr_enabled = false;
        cfg.max_pages = 7;
        let lp = LiteParse::new(cfg);
        assert!(!lp.config().ocr_enabled);
        assert_eq!(lp.config().max_pages, 7);
    }
}
