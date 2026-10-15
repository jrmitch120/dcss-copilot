//! Extracts a Game Knowledge bundle from a crawl checkout at a release tag.
//!
//! ```sh
//! gk-extract <crawl checkout> --tag 0.34.1 --commit <sha> --tiles app/static/tiles \
//!   > crates/gk/bundles/0.34.1.json
//! ```

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, bail};
use gk_extract::{CrawlRelease, copy_tiles, extract};

const USAGE: &str =
    "usage: gk-extract <crawl checkout> --tag <release tag> --commit <sha> [--tiles <dir>]";

fn main() -> anyhow::Result<()> {
    let mut crawl: Option<PathBuf> = None;
    let mut tag = None;
    let mut commit = None;
    let mut tiles: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tag" => tag = args.next(),
            "--commit" => commit = args.next(),
            "--tiles" => tiles = args.next().map(PathBuf::from),
            _ if crawl.is_none() && !arg.starts_with('-') => crawl = Some(arg.into()),
            _ => bail!("unexpected argument {arg}\n{USAGE}"),
        }
    }
    let (Some(crawl), Some(tag), Some(commit)) = (crawl, tag, commit) else {
        bail!(USAGE);
    };

    let release = CrawlRelease::checked_out(&crawl, &tag, &commit)?;
    let bundle = extract(&crawl, &release)
        .with_context(|| format!("extracting from {}", crawl.display()))?;
    if let Some(dir) = tiles {
        let copied = copy_tiles(&crawl, &bundle, &dir)
            .with_context(|| format!("copying tiles into {}", dir.display()))?;
        eprintln!("copied {copied} tiles into {}", dir.display());
    }
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &bundle)?;
    writeln!(out)?;
    Ok(())
}
