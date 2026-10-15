//! Reads and writes Recordings as plain JSON Lines, so they can be read, diffed and hand-built.
//!
//! ```sh
//! dcssrec unpack game.dcssrec              # the header, then one envelope per line
//! dcssrec pack scenario.jsonl game.dcssrec # envelopes, optionally after a header line
//! ```

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};

use anyhow::{Context, bail};
use copilot_replay::{RecordingHeader, RecordingWriter};
use copilot_webtiles::Envelope;

const USAGE: &str = "usage: dcssrec unpack <recording.dcssrec>\n       dcssrec pack <envelopes.jsonl> <recording.dcssrec>";

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["unpack", recording] => unpack(recording),
        ["pack", envelopes, recording] => pack(envelopes, recording),
        _ => bail!(USAGE),
    }
}

/// Prints every line as written, even from a Recording cut short.
fn unpack(path: &str) -> anyhow::Result<()> {
    let file = File::open(path).with_context(|| format!("can't open {path}"))?;
    let mut text = Vec::new();
    match zstd::Decoder::new(file)?.read_to_end(&mut text) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            eprintln!("{path} was cut short; printing what was saved");
        }
        Err(error) => return Err(error).with_context(|| format!("can't decompress {path}")),
    }
    std::io::stdout().lock().write_all(&text)?;
    Ok(())
}

/// Writes envelopes, one JSON object per line, as a Recording. A leading header line, as
/// `unpack` prints, keeps its app version; otherwise this build's version is used.
fn pack(envelopes: &str, recording: &str) -> anyhow::Result<()> {
    let input = File::open(envelopes).with_context(|| format!("can't open {envelopes}"))?;
    let mut lines = BufReader::new(input).lines().enumerate().peekable();
    let mut app_version = env!("CARGO_PKG_VERSION").to_owned();
    if let Some((_, Ok(first))) = lines.peek()
        && let Ok(header) = serde_json::from_str::<RecordingHeader>(first)
    {
        app_version = header.app_version;
        lines.next();
    }
    let output = File::create(recording).with_context(|| format!("can't create {recording}"))?;
    let mut writer = RecordingWriter::new(BufWriter::new(output), &app_version)?;
    for (index, line) in lines {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let envelope: Envelope = serde_json::from_str(&line)
            .with_context(|| format!("{envelopes}:{} is not an envelope", index + 1))?;
        writer.write(&envelope)?;
    }
    writer.finish()?.flush()?;
    Ok(())
}
