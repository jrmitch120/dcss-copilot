//! The DCSS Copilot desktop app: wires the core to its windows, tray and keychain.

use copilot_core::ViewModel;
use tauri_specta::{Builder, collect_commands};

/// Where the generated TypeScript bindings for the webview live.
const BINDINGS_PATH: &str = "../src/lib/bindings.ts";

#[tauri::command]
#[specta::specta]
fn view_model() -> ViewModel {
    ViewModel::initial(env!("CARGO_PKG_VERSION"))
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![view_model])
}

/// Writes the webview's TypeScript bindings. Run by the `bindings` test.
///
/// # Errors
/// If the bindings can't be generated or written.
pub fn export_bindings() -> Result<(), specta_typescript::Error> {
    specta_builder().export(specta_typescript::Typescript::default(), BINDINGS_PATH)
}

/// Starts the app.
///
/// # Panics
/// If Tauri fails to start, which leaves nothing to recover.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(clippy::expect_used)]
pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running DCSS Copilot");
}

#[cfg(test)]
mod tests {
    /// Regenerates the bindings; CI fails if this leaves the committed file changed.
    #[test]
    fn bindings() {
        super::export_bindings().unwrap();
    }
}
