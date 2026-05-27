//! Runtime loading of the pdfium shared library via `libloading`.
//!
//! On non-wasm targets, pdfium is loaded at runtime instead of being linked
//! at compile time. This avoids rpath issues when `liteparse` is used as a
//! library dependency in other Rust projects.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use libloading::Library;

use crate::*;

static BINDINGS: OnceLock<PdfiumBindings> = OnceLock::new();

/// The compile-time lib directory baked in by pdfium-sys's build.rs.
const PDFIUM_LIB_DIR: &str = env!("PDFIUM_LIB_DIR");

macro_rules! load_fn {
    ($lib:expr, $name:literal) => {{
        let sym = unsafe { $lib.get::<*const ()>($name.as_bytes())? };
        unsafe { std::mem::transmute(*sym) }
    }};
}

/// Holds all pdfium function pointers loaded at runtime.
pub struct PdfiumBindings {
    // Keep the library handle alive — dropping it would unload the symbols.
    _lib: Library,

    // -- Library lifecycle --
    pub FPDF_InitLibrary: unsafe extern "C" fn(),
    pub FPDF_GetLastError: unsafe extern "C" fn() -> std::os::raw::c_ulong,

    // -- Document --
    pub FPDF_LoadDocument: unsafe extern "C" fn(FPDF_STRING, FPDF_BYTESTRING) -> FPDF_DOCUMENT,
    pub FPDF_LoadMemDocument: unsafe extern "C" fn(
        *const std::os::raw::c_void,
        std::os::raw::c_int,
        FPDF_BYTESTRING,
    ) -> FPDF_DOCUMENT,
    pub FPDF_CloseDocument: unsafe extern "C" fn(FPDF_DOCUMENT),
    pub FPDF_GetPageCount: unsafe extern "C" fn(FPDF_DOCUMENT) -> std::os::raw::c_int,

    // -- Page --
    pub FPDF_LoadPage: unsafe extern "C" fn(FPDF_DOCUMENT, std::os::raw::c_int) -> FPDF_PAGE,
    pub FPDF_ClosePage: unsafe extern "C" fn(FPDF_PAGE),
    pub FPDF_GetPageWidthF: unsafe extern "C" fn(FPDF_PAGE) -> f32,
    pub FPDF_GetPageHeightF: unsafe extern "C" fn(FPDF_PAGE) -> f32,
    pub FPDF_GetPageBoundingBox: unsafe extern "C" fn(FPDF_PAGE, *mut FS_RECTF) -> FPDF_BOOL,
    pub FPDFPage_GetRotation: unsafe extern "C" fn(FPDF_PAGE) -> std::os::raw::c_int,
    pub FPDF_PageToDevice: unsafe extern "C" fn(
        FPDF_PAGE,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        f64,
        f64,
        *mut std::os::raw::c_int,
        *mut std::os::raw::c_int,
    ) -> FPDF_BOOL,

    // -- Page objects --
    pub FPDFPage_CountObjects: unsafe extern "C" fn(FPDF_PAGE) -> std::os::raw::c_int,
    pub FPDFPage_GetObject: unsafe extern "C" fn(FPDF_PAGE, std::os::raw::c_int) -> FPDF_PAGEOBJECT,
    pub FPDFPageObj_GetType: unsafe extern "C" fn(FPDF_PAGEOBJECT) -> std::os::raw::c_int,
    pub FPDFPageObj_GetBounds:
        unsafe extern "C" fn(FPDF_PAGEOBJECT, *mut f32, *mut f32, *mut f32, *mut f32) -> FPDF_BOOL,
    pub FPDFPageObj_GetMarkedContentID:
        unsafe extern "C" fn(FPDF_PAGEOBJECT) -> std::os::raw::c_int,
    pub FPDFImageObj_GetRenderedBitmap:
        unsafe extern "C" fn(FPDF_DOCUMENT, FPDF_PAGE, FPDF_PAGEOBJECT) -> FPDF_BITMAP,

    // -- TextPage --
    pub FPDFText_LoadPage: unsafe extern "C" fn(FPDF_PAGE) -> FPDF_TEXTPAGE,
    pub FPDFText_ClosePage: unsafe extern "C" fn(FPDF_TEXTPAGE),
    pub FPDFText_CountChars: unsafe extern "C" fn(FPDF_TEXTPAGE) -> std::os::raw::c_int,
    pub FPDFText_GetUnicode:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> std::os::raw::c_uint,
    pub FPDFText_GetCharCode:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> std::os::raw::c_uint,
    pub FPDFText_GetFontSize: unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> f64,
    pub FPDFText_GetFontWeight:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> std::os::raw::c_int,
    pub FPDFText_GetFontInfo: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        std::os::raw::c_int,
        *mut std::os::raw::c_void,
        std::os::raw::c_ulong,
        *mut std::os::raw::c_int,
    ) -> std::os::raw::c_ulong,
    pub FPDFText_GetCharAngle: unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> f32,
    pub FPDFText_GetCharBox: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        std::os::raw::c_int,
        *mut f64,
        *mut f64,
        *mut f64,
        *mut f64,
    ) -> FPDF_BOOL,
    pub FPDFText_GetLooseCharBox:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int, *mut FS_RECTF) -> FPDF_BOOL,
    pub FPDFText_GetMatrix:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int, *mut FS_MATRIX) -> FPDF_BOOL,
    pub FPDFText_IsGenerated:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> std::os::raw::c_int,
    pub FPDFText_HasUnicodeMapError:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> std::os::raw::c_int,
    pub FPDFText_GetTextObject:
        unsafe extern "C" fn(FPDF_TEXTPAGE, std::os::raw::c_int) -> FPDF_PAGEOBJECT,
    pub FPDFText_GetStrokeColor: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        std::os::raw::c_int,
        *mut std::os::raw::c_uint,
        *mut std::os::raw::c_uint,
        *mut std::os::raw::c_uint,
        *mut std::os::raw::c_uint,
    ) -> FPDF_BOOL,
    pub FPDFText_GetFillColor: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        std::os::raw::c_int,
        *mut std::os::raw::c_uint,
        *mut std::os::raw::c_uint,
        *mut std::os::raw::c_uint,
        *mut std::os::raw::c_uint,
    ) -> FPDF_BOOL,
    pub FPDFText_CountRects: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        std::os::raw::c_int,
        std::os::raw::c_int,
    ) -> std::os::raw::c_int,
    pub FPDFText_GetRect: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        std::os::raw::c_int,
        *mut f64,
        *mut f64,
        *mut f64,
        *mut f64,
    ) -> FPDF_BOOL,
    pub FPDFText_GetBoundedText: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        f64,
        f64,
        f64,
        f64,
        *mut std::os::raw::c_ushort,
        std::os::raw::c_int,
    ) -> std::os::raw::c_int,
    pub FPDFText_GetText: unsafe extern "C" fn(
        FPDF_TEXTPAGE,
        std::os::raw::c_int,
        std::os::raw::c_int,
        *mut std::os::raw::c_ushort,
    ) -> std::os::raw::c_int,
    pub FPDFTextObj_GetTextRenderMode:
        unsafe extern "C" fn(FPDF_PAGEOBJECT) -> FPDF_TEXT_RENDERMODE,

    // -- Bitmap --
    pub FPDFBitmap_CreateEx: unsafe extern "C" fn(
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        *mut std::os::raw::c_void,
        std::os::raw::c_int,
    ) -> FPDF_BITMAP,
    pub FPDFBitmap_Destroy: unsafe extern "C" fn(FPDF_BITMAP),
    pub FPDFBitmap_GetWidth: unsafe extern "C" fn(FPDF_BITMAP) -> std::os::raw::c_int,
    pub FPDFBitmap_GetHeight: unsafe extern "C" fn(FPDF_BITMAP) -> std::os::raw::c_int,
    pub FPDFBitmap_GetStride: unsafe extern "C" fn(FPDF_BITMAP) -> std::os::raw::c_int,
    pub FPDFBitmap_GetBuffer: unsafe extern "C" fn(FPDF_BITMAP) -> *mut std::os::raw::c_void,
    pub FPDFBitmap_FillRect: unsafe extern "C" fn(
        FPDF_BITMAP,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        FPDF_DWORD,
    ) -> FPDF_BOOL,

    // -- Rendering --
    pub FPDF_RenderPageBitmap: unsafe extern "C" fn(
        FPDF_BITMAP,
        FPDF_PAGE,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
        std::os::raw::c_int,
    ),

    // -- Font --
    pub FPDFTextObj_GetFont: unsafe extern "C" fn(FPDF_PAGEOBJECT) -> FPDF_FONT,
    pub FPDFFont_GetBaseFontName:
        unsafe extern "C" fn(FPDF_FONT, *mut std::os::raw::c_char, usize) -> usize,
    pub FPDFFont_GetType: unsafe extern "C" fn(FPDF_FONT) -> FPDF_FONT_TYPE,
    pub FPDFFont_GetIsEmbedded: unsafe extern "C" fn(FPDF_FONT) -> std::os::raw::c_int,
    pub FPDFFont_GetAscent: unsafe extern "C" fn(FPDF_FONT, f32, *mut f32) -> FPDF_BOOL,
    pub FPDFFont_GetDescent: unsafe extern "C" fn(FPDF_FONT, f32, *mut f32) -> FPDF_BOOL,
    pub FPDFFont_GetGlyphWidth: unsafe extern "C" fn(FPDF_FONT, u32, f32, *mut f32) -> FPDF_BOOL,
    pub FPDFFont_GetGlyphWidthFromCharCode:
        unsafe extern "C" fn(FPDF_FONT, u32, f32, *mut f32) -> FPDF_BOOL,
}

// SAFETY: PdfiumBindings contains only function pointers and a Library handle.
// The Library handle is thread-safe (it just holds a dlopen handle).
// Function pointers are inherently Send+Sync.
unsafe impl Send for PdfiumBindings {}
unsafe impl Sync for PdfiumBindings {}

impl PdfiumBindings {
    fn load(lib: Library) -> Result<Self, libloading::Error> {
        Ok(Self {
            FPDF_InitLibrary: load_fn!(lib, "FPDF_InitLibrary"),
            FPDF_GetLastError: load_fn!(lib, "FPDF_GetLastError"),
            FPDF_LoadDocument: load_fn!(lib, "FPDF_LoadDocument"),
            FPDF_LoadMemDocument: load_fn!(lib, "FPDF_LoadMemDocument"),
            FPDF_CloseDocument: load_fn!(lib, "FPDF_CloseDocument"),
            FPDF_GetPageCount: load_fn!(lib, "FPDF_GetPageCount"),
            FPDF_LoadPage: load_fn!(lib, "FPDF_LoadPage"),
            FPDF_ClosePage: load_fn!(lib, "FPDF_ClosePage"),
            FPDF_GetPageWidthF: load_fn!(lib, "FPDF_GetPageWidthF"),
            FPDF_GetPageHeightF: load_fn!(lib, "FPDF_GetPageHeightF"),
            FPDF_GetPageBoundingBox: load_fn!(lib, "FPDF_GetPageBoundingBox"),
            FPDFPage_GetRotation: load_fn!(lib, "FPDFPage_GetRotation"),
            FPDF_PageToDevice: load_fn!(lib, "FPDF_PageToDevice"),
            FPDFPage_CountObjects: load_fn!(lib, "FPDFPage_CountObjects"),
            FPDFPage_GetObject: load_fn!(lib, "FPDFPage_GetObject"),
            FPDFPageObj_GetType: load_fn!(lib, "FPDFPageObj_GetType"),
            FPDFPageObj_GetBounds: load_fn!(lib, "FPDFPageObj_GetBounds"),
            FPDFPageObj_GetMarkedContentID: load_fn!(lib, "FPDFPageObj_GetMarkedContentID"),
            FPDFImageObj_GetRenderedBitmap: load_fn!(lib, "FPDFImageObj_GetRenderedBitmap"),
            FPDFText_LoadPage: load_fn!(lib, "FPDFText_LoadPage"),
            FPDFText_ClosePage: load_fn!(lib, "FPDFText_ClosePage"),
            FPDFText_CountChars: load_fn!(lib, "FPDFText_CountChars"),
            FPDFText_GetUnicode: load_fn!(lib, "FPDFText_GetUnicode"),
            FPDFText_GetCharCode: load_fn!(lib, "FPDFText_GetCharCode"),
            FPDFText_GetFontSize: load_fn!(lib, "FPDFText_GetFontSize"),
            FPDFText_GetFontWeight: load_fn!(lib, "FPDFText_GetFontWeight"),
            FPDFText_GetFontInfo: load_fn!(lib, "FPDFText_GetFontInfo"),
            FPDFText_GetCharAngle: load_fn!(lib, "FPDFText_GetCharAngle"),
            FPDFText_GetCharBox: load_fn!(lib, "FPDFText_GetCharBox"),
            FPDFText_GetLooseCharBox: load_fn!(lib, "FPDFText_GetLooseCharBox"),
            FPDFText_GetMatrix: load_fn!(lib, "FPDFText_GetMatrix"),
            FPDFText_IsGenerated: load_fn!(lib, "FPDFText_IsGenerated"),
            FPDFText_HasUnicodeMapError: load_fn!(lib, "FPDFText_HasUnicodeMapError"),
            FPDFText_GetTextObject: load_fn!(lib, "FPDFText_GetTextObject"),
            FPDFText_GetStrokeColor: load_fn!(lib, "FPDFText_GetStrokeColor"),
            FPDFText_GetFillColor: load_fn!(lib, "FPDFText_GetFillColor"),
            FPDFText_CountRects: load_fn!(lib, "FPDFText_CountRects"),
            FPDFText_GetRect: load_fn!(lib, "FPDFText_GetRect"),
            FPDFText_GetBoundedText: load_fn!(lib, "FPDFText_GetBoundedText"),
            FPDFText_GetText: load_fn!(lib, "FPDFText_GetText"),
            FPDFTextObj_GetTextRenderMode: load_fn!(lib, "FPDFTextObj_GetTextRenderMode"),
            FPDFBitmap_CreateEx: load_fn!(lib, "FPDFBitmap_CreateEx"),
            FPDFBitmap_Destroy: load_fn!(lib, "FPDFBitmap_Destroy"),
            FPDFBitmap_GetWidth: load_fn!(lib, "FPDFBitmap_GetWidth"),
            FPDFBitmap_GetHeight: load_fn!(lib, "FPDFBitmap_GetHeight"),
            FPDFBitmap_GetStride: load_fn!(lib, "FPDFBitmap_GetStride"),
            FPDFBitmap_GetBuffer: load_fn!(lib, "FPDFBitmap_GetBuffer"),
            FPDFBitmap_FillRect: load_fn!(lib, "FPDFBitmap_FillRect"),
            FPDF_RenderPageBitmap: load_fn!(lib, "FPDF_RenderPageBitmap"),
            FPDFTextObj_GetFont: load_fn!(lib, "FPDFTextObj_GetFont"),
            FPDFFont_GetBaseFontName: load_fn!(lib, "FPDFFont_GetBaseFontName"),
            FPDFFont_GetType: load_fn!(lib, "FPDFFont_GetType"),
            FPDFFont_GetIsEmbedded: load_fn!(lib, "FPDFFont_GetIsEmbedded"),
            FPDFFont_GetAscent: load_fn!(lib, "FPDFFont_GetAscent"),
            FPDFFont_GetDescent: load_fn!(lib, "FPDFFont_GetDescent"),
            FPDFFont_GetGlyphWidth: load_fn!(lib, "FPDFFont_GetGlyphWidth"),
            FPDFFont_GetGlyphWidthFromCharCode: load_fn!(lib, "FPDFFont_GetGlyphWidthFromCharCode"),
            _lib: lib,
        })
    }
}

/// Shared library file name for the current platform.
fn dylib_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "libpdfium.dylib"
    }
    #[cfg(target_os = "windows")]
    {
        "pdfium.dll"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "libpdfium.so"
    }
}

/// Search paths for the pdfium shared library, in priority order.
fn search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let name = dylib_name();

    // 1. Runtime env var override (directory containing the shared library)
    if let Ok(dir) = std::env::var("PDFIUM_LIB_PATH") {
        paths.push(PathBuf::from(&dir).join(name));
    }

    // 2. Compile-time baked path from build.rs
    if !PDFIUM_LIB_DIR.is_empty() {
        let lib_dir = PathBuf::from(PDFIUM_LIB_DIR);
        paths.push(lib_dir.join(name));

        // On Windows, pdfium-binaries puts the DLL in bin/, not lib/
        #[cfg(target_os = "windows")]
        if let Some(parent) = lib_dir.parent() {
            paths.push(parent.join("bin").join(name));
        }
    }

    // 3. Next to the current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            paths.push(exe_dir.join(name));
        }
    }

    // 4. Bare library name (system search paths / LD_LIBRARY_PATH / DYLD_LIBRARY_PATH / PATH)
    paths.push(PathBuf::from(name));

    paths
}

/// Load the pdfium shared library from a specific path.
pub fn load(lib_path: &Path) -> Result<(), String> {
    if BINDINGS.get().is_some() {
        return Ok(());
    }
    let lib = unsafe { Library::new(lib_path) }
        .map_err(|e| format!("failed to load pdfium from {}: {e}", lib_path.display()))?;
    let bindings =
        PdfiumBindings::load(lib).map_err(|e| format!("failed to resolve pdfium symbols: {e}"))?;
    let _ = BINDINGS.set(bindings);
    Ok(())
}

/// Load the pdfium shared library from default search paths.
///
/// Search order:
/// 1. `PDFIUM_LIB_PATH` env var (directory containing the shared library)
/// 2. Compile-time cached download path
/// 3. System library search paths
pub fn load_default() -> Result<(), String> {
    if BINDINGS.get().is_some() {
        return Ok(());
    }

    let paths = search_paths();
    let mut last_err = String::from("no search paths configured");

    for path in &paths {
        match unsafe { Library::new(path) } {
            Ok(lib) => match PdfiumBindings::load(lib) {
                Ok(bindings) => {
                    let _ = BINDINGS.set(bindings);
                    return Ok(());
                }
                Err(e) => {
                    last_err = format!(
                        "failed to resolve pdfium symbols from {}: {e}",
                        path.display()
                    );
                }
            },
            Err(e) => {
                last_err = format!("{}: {e}", path.display());
            }
        }
    }

    Err(format!(
        "could not find pdfium shared library. Last error: {last_err}. \
         Set PDFIUM_LIB_PATH to the directory containing {}",
        dylib_name()
    ))
}

/// Get a reference to the loaded pdfium bindings.
///
/// # Panics
/// Panics if `load()` or `load_default()` has not been called successfully.
pub fn pdfium() -> &'static PdfiumBindings {
    BINDINGS
        .get()
        .expect("pdfium not loaded — call pdfium_sys::dynamic::load_default() first")
}
