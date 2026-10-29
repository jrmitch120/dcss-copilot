//! The Coach's calls to the Quick Model: one in flight at a time, the latest, its answer handed to
//! the live game as it arrives.

use std::sync::{Mutex, PoisonError};

use copilot_llm::ModelSlot;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Manager};

use crate::live::Live;
use crate::models::ModelsAndKeys;

/// The Coach's call in flight, if any.
#[derive(Default)]
pub struct CoachCalls {
    in_flight: Mutex<Option<JoinHandle<()>>>,
}

impl CoachCalls {
    /// Makes the call for the Coach's latest question, if one waits, and cancels any still in
    /// flight, whose answer the game would drop. The answer reaches the panel when it arrives.
    pub fn ask(&self, app: &AppHandle) {
        let live = app.state::<Live>();
        let Some(call) = live.take_call() else {
            return;
        };
        let provider = live.model(ModelSlot::Quick).provider();
        let sidecar = live.llm_sidecar();
        let app = app.clone();
        let task = tauri::async_runtime::spawn(async move {
            let response = app
                .state::<ModelsAndKeys>()
                .ask(provider, call.request, sidecar)
                .await;
            let live = app.state::<Live>();
            live.answer(call.question, response);
            crate::push_view_model(&app, &live);
        });
        let replaced = self
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(task);
        if let Some(replaced) = replaced {
            replaced.abort();
        }
    }
}
