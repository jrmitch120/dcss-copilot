//! Builds and fetches the test corpus. See `docs/corpus.md`.
//!
//! ```sh
//! cargo run -p corpus -- fetch
//! cargo run -p corpus -- record-scenario crates/replay/tests/scenarios/<name>.toml
//! cargo run -p corpus -- record-qw path/to/qw-final.rc --seed 1
//! ```

mod capture;
mod session;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use copilot_replay::{Corpus, Recording, Replayer, Scenario, sha256_hex};
use tokio::time::Instant;

use crate::session::{Session, Stopped};

const USAGE: &str = "\
usage:
  corpus fetch [--cache <dir>]
  corpus record-scenario <scenario.toml> [--server <ws://…/socket>] [--game <id>]
  corpus record-qw <qw-final.rc> --seed <n> [--server <ws://…/socket>] [--game <id>] [--minutes <n>] [--out-dir <dir>]";

/// Local webtiles, run from crawl's source directory as `webserver/server.py` does by default.
const DEFAULT_SERVER: &str = "ws://localhost:8080/socket";
/// The game entries docs/corpus.md adds to the local server's `games.d`.
const DEFAULT_WIZARD_GAME: &str = "copilot-wizard";
const DEFAULT_QW_GAME: &str = "copilot-qw";
/// Local accounts only, so the password guards nothing.
const PASSWORD: &str = "corpus";
/// The longest a scenario may take to replay its keys.
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The longest a qw game is recorded by default, so a stuck bot still yields a Recording.
const DEFAULT_QW_MINUTES: u64 = 120;
/// Escape: dismisses a `--more--` or a menu qw has left open, such as after it dies.
const ESCAPE: &str = "\u{1b}";

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
            let game = options
                .take("--game")
                .unwrap_or(DEFAULT_WIZARD_GAME.to_owned());
            options.done()?;
            record_scenario(Path::new(&manifest), &server, &game).await
        }
        "record-qw" => {
            let rc = options.positional()?;
            let seed: u64 = options
                .take("--seed")
                .context("record-qw needs --seed")?
                .parse()
                .context("--seed must be a whole number")?;
            let server = options
                .take("--server")
                .unwrap_or(DEFAULT_SERVER.to_owned());
            let game = options.take("--game").unwrap_or(DEFAULT_QW_GAME.to_owned());
            let minutes: u64 = options
                .take("--minutes")
                .map_or(Ok(DEFAULT_QW_MINUTES), |minutes| minutes.parse())
                .context("--minutes must be a whole number")?;
            let out_dir = options
                .take("--out-dir")
                .map_or_else(|| Corpus::cache_dir().join("recorded"), PathBuf::from);
            options.done()?;
            let rc = std::fs::read_to_string(&rc).with_context(|| format!("couldn't read {rc}"))?;
            record_qw(&rc, seed, &server, &game, minutes, &out_dir).await
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

/// Plays a scenario's setup on a fresh account and records it over the scenario's Recording.
async fn record_scenario(manifest: &Path, server: &str, game: &str) -> anyhow::Result<()> {
    let scenario = Scenario::load(manifest)?;
    let mut session = Session::connect(server, &scenario.recording).await?;
    session.register(&account("wiz"), PASSWORD).await?;
    session.set_rc(game, &scenario.setup.rc).await?;
    session.play(game).await?;
    let deadline = Instant::now() + SCENARIO_TIMEOUT;
    session.settle(deadline).await?;
    for keys in &scenario.setup.keys {
        session.input(keys).await?;
        match session.settle(deadline).await? {
            Stopped::Settled => {}
            Stopped::GameOver => bail!("the game ended before the keys {keys:?}"),
            Stopped::Deadline => bail!("the scenario took longer than {SCENARIO_TIMEOUT:?}"),
        }
    }
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

/// Records qw playing one seeded game, then prints its corpus manifest entry.
async fn record_qw(
    rc: &str,
    seed: u64,
    server: &str,
    game: &str,
    minutes: u64,
    out_dir: &Path,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("couldn't create {}", out_dir.display()))?;
    let partial = out_dir.join(format!("qw-seed-{seed}.dcssrec.part"));
    let mut session = Session::connect(server, &partial).await?;
    session.register(&account("qw"), PASSWORD).await?;
    // qw starts itself and plays at full speed; the seed picks the dungeon.
    let rc = format!("{rc}\n: AUTO_START = true\n: DELAYED = false\ngame_seed = {seed}\n");
    session.set_rc(game, &rc).await?;
    session.play(game).await?;
    let deadline = Instant::now() + Duration::from_secs(minutes * 60);
    let ended = loop {
        match session.settle(deadline).await? {
            // qw acts as soon as crawl asks for a command, so a quiet game waits on a prompt.
            Stopped::Settled => session.input(ESCAPE).await?,
            stopped => break stopped,
        }
        // qw stops for good on a Lua error, such as on a Crawl Version it doesn't support yet.
        if let Some(error) = session.lua_error().map(str::to_owned) {
            session.finish().await?;
            bail!(
                "qw failed, so {} isn't a corpus game: {error}",
                partial.display()
            );
        }
    };
    session.finish().await?;
    if ended == Stopped::Deadline {
        eprintln!("qw was still playing after {minutes} minutes: the Recording stops there");
    }

    let replayer = open_replayer(&partial)?;
    let versions = replayer.crawl_versions();
    let [version] = versions else {
        bail!(
            "{} reports Crawl Versions {versions:?}, not one",
            partial.display()
        );
    };
    let file = format!("qw-{version}-seed-{seed}.dcssrec");
    let path = out_dir.join(&file);
    std::fs::rename(&partial, &path)?;
    let bytes = std::fs::read(&path)?;
    println!(
        "# {}\n[[game]]\nfile = \"{file}\"\nsha256 = \"{}\"\ncrawl_version = \"{version}\"\nplayed_by = \"qw\"\nseed = {seed}\n",
        path.display(),
        sha256_hex(&bytes)
    );
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
