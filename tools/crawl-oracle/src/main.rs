//! Diffs the engine's ported formulas against a crawl build: reads what the harness printed and
//! fails if the engine disagrees with crawl on any case.
//!
//! Usage: `crawl-oracle <dump>`, or `crawl-oracle -` to read the dump from standard input.

use std::io::Read;
use std::process::ExitCode;

use anyhow::Context;

fn main() -> anyhow::Result<ExitCode> {
    let path = std::env::args()
        .nth(1)
        .context("usage: crawl-oracle <dump>, or - for standard input")?;
    let dump = if path == "-" {
        let mut dump = String::new();
        std::io::stdin()
            .read_to_string(&mut dump)
            .context("reading the dump from standard input")?;
        dump
    } else {
        std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?
    };
    let report = crawl_oracle::diff(&dump)?;
    print!("{report}");
    Ok(if report.mismatches.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
