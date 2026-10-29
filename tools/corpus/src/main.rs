//! Builds and fetches the test corpus. See `docs/corpus.md`.
//!
//! ```sh
//! cargo run -p corpus -- fetch
//! cargo run -p corpus -- record-scenario crates/replay/tests/scenarios/<name>.toml
//! ```

mod capture;
mod session;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use copilot_replay::{Corpus, Recording, Replayer, Scenario};
use tokio::time::Instant;

use crate::session::{Session, Stopped};

const USAGE: &str = "\
usage:
  corpus fetch [--cache <dir>]
  corpus record-scenario <scenario.toml> [--server <ws://…/socket>] [--game <id>]";

/// Local webtiles, run from crawl's source directory as `webserver/server.py` does by default.
const DEFAULT_SERVER: &str = "ws://localhost:8080/socket";
/// The game entry docs/corpus.md adds to the local server's `games.d`.
const DEFAULT_GAME: &str = "copilot-wizard";
/// Local accounts only, so the password guards nothing.
const PASSWORD: &str = "corpus";
/// The longest a scenario may take to replay its keys.
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        bail!("{USAGE}");
    };
    let mut options = Options::parse(rest)?;
    match command.as_str() {
        "fetch" => {
            let cache = options
                .take("--cache")
                .map_or_else(Corpus::cache_dir, PathBuf::from);
            options.done()?;
            fetch(&cache).await
        }
        "record-scenario" => {
            let manifest = options.positional()?;
            let server = options
                .take("--server")
                .unwrap_or(DEFAULT_SERVER.to_owned());
            let game = options.take("--game").unwrap_or(DEFAULT_GAME.to_owned());
            options.done()?;
            record_scenario(Path::new(&manifest), &server, &game).await
        }
        _ => bail!("unknown command {command:?}\n{USAGE}"),
    }
}

/// Downloads every corpus game not yet cached, checking each against its pinned hash.
async fn fetch(cache: &Path) -> anyhow::Result<()> {
    let corpus = Corpus::committed()?;
    std::fs::create_dir_all(cache)
        .with_context(|| format!("couldn't create {}", cache.display()))?;
    let client = reqwest::Client::new();
    for game in &corpus.games {
        if game.read_cached(cache).is_ok() {
            println!("cached      {}", game.file);
            continue;
        }
        let url = corpus.url(game);
        let bytes = client
            .get(&url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .with_context(|| format!("couldn't fetch {url}"))?
            .bytes()
            .await
            .with_context(|| format!("couldn't fetch {url}"))?;
        game.check(&bytes)?;
        let path = game.cached_path(cache);
        let partial = path.with_extension("part");
        std::fs::write(&partial, &bytes)
            .and_then(|()| std::fs::rename(&partial, &path))
            .with_context(|| format!("couldn't write {}", path.display()))?;
        println!("fetched     {}", game.file);
    }
    println!("{} corpus games in {}", corpus.games.len(), cache.display());
    Ok(())
}

/// Plays a scenario's setup on a fresh account and records it over the scenario's Recording: from
/// the start, or from when the game is played again after it was played and saved unrecorded.
async fn record_scenario(manifest: &Path, server: &str, game: &str) -> anyhow::Result<()> {
    let scenario = Scenario::load(manifest)?;
    let setup = &scenario.setup;
    let username = account("wiz");
    let deadline = Instant::now() + SCENARIO_TIMEOUT;
    let mut session = if setup.before_joining.is_empty() {
        let mut session = Session::connect(server, Some(&scenario.recording)).await?;
        session.register(&username, PASSWORD).await?;
        session.set_rc(game, &setup.rc).await?;
        session
    } else {
        let mut unrecorded = Session::connect(server, None).await?;
        unrecorded.register(&username, PASSWORD).await?;
        unrecorded.set_rc(game, &setup.rc).await?;
        unrecorded.play(game).await?;
        type_keys(&mut unrecorded, &setup.before_joining, deadline).await?;
        unrecorded.finish().await?;
        let mut session = Session::connect(server, Some(&scenario.recording)).await?;
        session.log_in(&username, PASSWORD).await?;
        session
    };
    session.play(game).await?;
    type_keys(&mut session, &setup.keys, deadline).await?;
    session.finish().await?;

    open_replayer(&scenario.recording)?
        .check_crawl_version(&scenario.crawl_version)
        .with_context(|| {
            format!(
                "{} doesn't match the server: update its crawl_version",
                manifest.display()
            )
        })?;
    println!("recorded {}", scenario.recording.display());
    Ok(())
}

/// Types each of `keys` once the game has settled from the one before, from a game just started.
/// The game may stop after the last, as when it saves or the character dies, but not before.
async fn type_keys(
    session: &mut Session,
    keys: &[String],
    deadline: Instant,
) -> anyhow::Result<()> {
    session.settle(deadline).await?;
    for (typed, key) in keys.iter().enumerate() {
        session.input(key).await?;
        let last = typed + 1 == keys.len();
        match session.settle(deadline).await? {
            Stopped::Settled => {}
            Stopped::GameOver if last => {}
            Stopped::GameOver => bail!("the game stopped after the keys {key:?}, before the rest"),
            Stopped::Deadline => bail!("the scenario took longer than {SCENARIO_TIMEOUT:?}"),
        }
    }
    Ok(())
}

/// The Recording at `path`, ready to replay.
fn open_replayer(path: &Path) -> anyhow::Result<Replayer> {
    let file =
        std::fs::File::open(path).with_context(|| format!("couldn't read {}", path.display()))?;
    Ok(Replayer::new(Recording::read(file)?)?)
}

/// A new account name: crawl allows letters and digits, at most 15 of them.
fn account(prefix: &str) -> String {
    format!("{prefix}{}", session::now_ms() % 1_000_000_000)
}

/// Command-line arguments: one positional and `--name value` options, each used once.
struct Options {
    positional: Vec<String>,
    named: Vec<(String, String)>,
}

impl Options {
    fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut options = Self {
            positional: Vec::new(),
            named: Vec::new(),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if arg.starts_with("--") {
                let value = args
                    .next()
                    .with_context(|| format!("{arg} needs a value\n{USAGE}"))?;
                options.named.push((arg.clone(), value.clone()));
            } else {
                options.positional.push(arg.clone());
            }
        }
        Ok(options)
    }

    fn take(&mut self, name: &str) -> Option<String> {
        let at = self.named.iter().position(|(named, _)| named == name)?;
        Some(self.named.remove(at).1)
    }

    fn positional(&mut self) -> anyhow::Result<String> {
        if self.positional.is_empty() {
            bail!("missing an argument\n{USAGE}");
        }
        Ok(self.positional.remove(0))
    }

    fn done(self) -> anyhow::Result<()> {
        if let Some(extra) = self.positional.first() {
            bail!("unexpected argument {extra:?}\n{USAGE}");
        }
        if let Some((name, _)) = self.named.first() {
            bail!("unknown option {name}\n{USAGE}");
        }
        Ok(())
    }
}
