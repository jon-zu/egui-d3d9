#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::cast_precision_loss
)]


mod app;
mod inputman;
mod mesh;
mod state;
mod texman;

use std::sync::Mutex;

pub use app::*;
use clipboard::ClipboardProvider;

static CLIPBOARD: Mutex<Option<clipboard::ClipboardContext>> = Mutex::new(None);

pub(crate) fn get_clipboard_text() -> Result<String, Box<dyn std::error::Error>> {
    let mut ctx = CLIPBOARD.lock().unwrap();
    if ctx.is_none() {
        *ctx = Some(clipboard::ClipboardContext::new()?);
    }

    ctx.as_mut().unwrap().get_contents()
}

pub(crate) fn set_clipboard_text(s: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = CLIPBOARD.lock().unwrap();
    if ctx.is_none() {
        *ctx = Some(clipboard::ClipboardContext::new()?);
    }

    ctx.as_mut().unwrap().set_contents(s)
}
