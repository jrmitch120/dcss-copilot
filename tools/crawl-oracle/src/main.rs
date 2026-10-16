//! Diffs the engine's ported formulas against a crawl build: reads what the harness printed and
//! fails if the engine disagrees with crawl on any case.
//!
//! Usage: `crawl-oracle <dump>`, or `crawl-oracle -` to read the dump from standard input.
//! `crawl-oracle ported-spells <crawl version>` prints the `spell_type` value of each spell the
//! engine ports, one per line, for the spell harness to draw its cases from.

use std::io::Read;
use std::process::ExitCode;

use anyhow::Context;

fn main() -> anyhow::Result<ExitCode> {
    let usage = "usage: crawl-oracle <dump>, or - for standard input, \
                 or crawl-oracle ported-spells <crawl version>";
    let mut args = std::env::args().skip(1);
    let path = args.next().context(usage)?;
    if path == "ported-spells" {
        let version = args.next().context(usage)?;
        for spell in crawl_oracle::ported_spells(&version)? {
            println!("{spell}");
        }
        return Ok(ExitCode::SUCCESS);
    }
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
