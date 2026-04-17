//! Python bindings for kham-core via PyO3.
//!
//! Build with:
//! ```bash
//! maturin develop -m kham-python/Cargo.toml
//! ```
//!
//! Then from Python:
//! ```python
//! import kham
//! tokens = kham.segment("กินข้าวกับปลา")
//! print(tokens)  # ['กิน', 'ข้าว', 'กับ', 'ปลา']
//! ```

use kham_core::Tokenizer;
use pyo3::prelude::*;

/// Segment Thai text into a list of token strings.
///
/// Args:
///     text (str): Input text (may contain mixed scripts).
///
/// Returns:
///     list[str]: Segmented tokens.
///
/// Example:
///     >>> import kham
///     >>> kham.segment("กินข้าวกับปลา")
///     ['กิน', 'ข้าว', 'กับ', 'ปลา']
#[pyfunction]
fn segment(text: &str) -> Vec<String> {
    let tok = Tokenizer::new();
    tok.segment(text)
        .into_iter()
        .map(|t| t.text.to_owned())
        .collect()
}

/// kham — Thai word segmentation engine.
#[pymodule]
fn kham(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(segment, m)?)?;
    Ok(())
}
