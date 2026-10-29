//! Records the LLM sidecar of each Recording given: the Recording is replayed at Coach, and every
//! call the Coach makes goes to the Quick Model by default, over the Anthropic key in
//! `ANTHROPIC_API_KEY`, and is written beside the Recording, replacing any sidecar there.
//!
//! Any change to a prompt changes its request's hash, so the replays that answer from a sidecar
//! fail with `NotRecorded` until it is recorded again:
//!
//! ```sh
//! ANTHROPIC_API_KEY=... cargo run -p copilot-replay --bin coach-sidecar -- crates/replay/tests/scenarios/ogre-corners-a-minotaur-carrying-escapes.dcssrec
//! ```

use std::fs::File;
use std::path::Path;

use anyhow::{Context, bail};
use copilot_llm::{
    AnthropicProvider, ApiKey, LlmProvider, LlmRequest, LlmStream, ModelChoice, ModelSlot,
    SidecarRecorder, sidecar_path,
};
use copilot_replay::{Recording, Replayer};
use futures_util::StreamExt;
use futures_util::stream;
use tokio::runtime::{Builder, Runtime};

const USAGE: &str = "usage: ANTHROPIC_API_KEY=... coach-sidecar <recording.dcssrec>...";

fn main() -> anyhow::Result<()> {
    let recordings: Vec<String> = std::env::args().skip(1).collect();
    if recordings.is_empty() {
        bail!(USAGE);
    }
    let key = std::env::var("ANTHROPIC_API_KEY").context(USAGE)?;
    for recording in &recordings {
        record(Path::new(recording), &key)?;
    }
    Ok(())
}

/// Replays `path` at Coach, recording each call into its sidecar.
fn record(path: &Path, key: &str) -> anyhow::Result<()> {
    let sidecar = sidecar_path(path);
    if sidecar.exists() {
        std::fs::remove_file(&sidecar)
            .with_context(|| format!("can't replace {}", sidecar.display()))?;
    }
    let live = Blocking {
        inner: AnthropicProvider::new(ApiKey::new(key)),
        runtime: Builder::new_current_thread().enable_all().build()?,
    };
    let recorder = SidecarRecorder::create(live, &sidecar)
        .with_context(|| format!("can't write {}", sidecar.display()))?;
    let file = File::open(path).with_context(|| format!("can't open {}", path.display()))?;
    let mut replayer = Replayer::new(Recording::read(file)?)?
        .coaching(recorder, ModelChoice::default_for(ModelSlot::Quick));
    replayer.seek(usize::MAX);
    eprintln!(
        "{}: {} calls recorded",
        sidecar.display(),
        replayer.calls().len()
    );
    Ok(())
}

/// A live provider made to answer at once, as a replay needs: each call runs to its end before
/// its stream is handed back.
struct Blocking<P> {
    inner: P,
    runtime: Runtime,
}

impl<P: LlmProvider> LlmProvider for Blocking<P> {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        let events: Vec<_> = self
            .runtime
            .block_on(self.inner.stream(request).collect::<Vec<_>>());
        stream::iter(events).boxed()
    }
}
