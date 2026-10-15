//! Plays a Recording to the running app as if a browser extension were relaying a live game.
//!
//! ```sh
//! cargo run -p copilot-replay --bin fake-extension -- game.dcssrec [--speed 4] [--play]
//! ```
//!
//! Commands are typed one per line; see `COMMANDS`.

use std::fs::File;
use std::io::BufReader;

use anyhow::{Context, bail};
use copilot_replay::{
    Applied, FAKE_EXTENSION_ORIGIN, FakeExtension, FakeExtensionError, Recording, Speed, app_url,
};
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio::time::{Instant, sleep_until};

const USAGE: &str = "usage: fake-extension <recording.dcssrec> [--speed <1|4|0.5|max>] [--play] [--url <ws://…>] [--origin <chrome-extension://…>]";

const COMMANDS: &str = "\
  p or Enter  play or pause
  s           step one message
  t           step one turn
  j <turn>    jump to the first message at that turn or later
  g <n>       go to just after message n (0 is the start)
  x <speed>   set the speed: 1 is real time, 4 is four times faster, max doesn't wait
  c           connect again, replaying everything so far
  h           show these commands
  q           quit";

struct Args {
    recording: String,
    speed: Speed,
    play: bool,
    url: String,
    origin: String,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut recording = None;
    let mut parsed = Args {
        recording: String::new(),
        speed: Speed::Times(1.0),
        play: false,
        url: app_url(),
        origin: FAKE_EXTENSION_ORIGIN.to_owned(),
    };
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .with_context(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--speed" => parsed.speed = value()?.parse().map_err(anyhow::Error::msg)?,
            "--play" => parsed.play = true,
            "--url" => parsed.url = value()?,
            "--origin" => parsed.origin = value()?,
            _ if recording.is_none() && !arg.starts_with("--") => recording = Some(arg),
            _ => bail!("unexpected argument {arg:?}\n{USAGE}"),
        }
    }
    parsed.recording = recording.context(USAGE)?;
    Ok(parsed)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let file =
        File::open(&args.recording).with_context(|| format!("can't open {}", args.recording))?;
    let recording = Recording::read(BufReader::new(file))?;
    if recording.truncated {
        println!("This Recording was cut short; it plays up to where it ends.");
    }
    let mut extension = FakeExtension::new(recording, &args.url, &args.origin)?;

    println!("{COMMANDS}");
    println!(
        "Connecting to {} as {}. If the app asks \"Allow this browser?\", answer it.",
        args.url, args.origin
    );
    report_connect(extension.connect().await);
    status(&extension);

    let mut player = Player {
        speed: args.speed,
        playing: args.play,
        anchor: None,
    };
    let mut lines = AsyncBufReader::new(tokio::io::stdin()).lines();
    loop {
        let due = player.next_due(&extension);
        tokio::select! {
            line = lines.next_line() => {
                player.anchor = None;
                let Some(line) = line? else { break };
                if !command(line.trim(), &mut extension, &mut player).await {
                    break;
                }
            }
            () = sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => {
                let turn = extension.replayer().turn();
                report_move(extension.step().await);
                if extension.replayer().at_end() {
                    player.playing = false;
                    println!("End of the Recording.");
                    status(&extension);
                } else if extension.replayer().turn() != turn {
                    status(&extension);
                }
            }
        }
    }
    extension.disconnect().await;
    Ok(())
}

/// Plays envelopes as their timestamps say, scaled by the speed.
struct Player {
    speed: Speed,
    playing: bool,
    /// When playing started or last changed: the wall-clock instant and the envelope clock then.
    anchor: Option<(Instant, u64)>,
}

impl Player {
    /// When the next envelope is due, if playing.
    fn next_due(&mut self, extension: &FakeExtension) -> Option<Instant> {
        if !self.playing {
            return None;
        }
        let page = extension.replayer();
        let next = page.next_envelope()?.ts;
        let (started, from_ms) = *self
            .anchor
            .get_or_insert_with(|| (Instant::now(), page.clock_ms().unwrap_or(next)));
        Some(started + self.speed.wait(from_ms, next))
    }
}

/// Carries out one typed command. Returns `false` to quit.
async fn command(line: &str, extension: &mut FakeExtension, player: &mut Player) -> bool {
    let (verb, argument) = line.split_once(' ').unwrap_or((line, ""));
    let argument = argument.trim();
    match verb {
        "" | "p" => {
            player.playing = !player.playing && !extension.replayer().at_end();
            println!(
                "{}",
                if player.playing {
                    "Playing."
                } else {
                    "Paused."
                }
            );
        }
        "s" => report_move(extension.step().await),
        "t" => report_move(extension.step_turn().await),
        "j" => match argument.parse() {
            Ok(turn) => match extension.seek_turn(turn).await {
                Ok(Some(applied)) => report_move(Ok(applied)),
                Ok(None) => println!("The Recording never reaches turn {turn}."),
                Err(error) => report_move(Err(error)),
            },
            Err(_) => println!("j needs a turn number"),
        },
        "g" => match argument.parse() {
            Ok(position) => report_move(extension.seek(position).await),
            Err(_) => println!("g needs a message number"),
        },
        "x" => match argument.parse() {
            Ok(speed) => {
                player.speed = speed;
                println!("Speed: {argument}");
            }
            Err(error) => println!("{error}"),
        },
        "c" => report_connect(extension.connect().await),
        "h" => println!("{COMMANDS}"),
        "q" => return false,
        _ => println!("Unknown command {verb:?}; h shows the commands."),
    }
    if !player.playing {
        status(extension);
    }
    true
}

fn report_connect(connected: Result<(), FakeExtensionError>) {
    match connected {
        Ok(()) => println!("Connected; everything so far was replayed to the app."),
        Err(error) => println!("Not connected: {error}. Moves carry on; c connects again."),
    }
}

fn report_move(moved: Result<Applied, FakeExtensionError>) {
    match moved {
        Ok(Applied {
            restarted: true, ..
        }) => println!("Went back: replayed from the start, as a reloaded page would."),
        Ok(_) => {}
        Err(error) => println!("Relay failed: {error}. Moves carry on; c connects again."),
    }
}

fn status(extension: &FakeExtension) {
    let page = extension.replayer();
    let envelopes = &page.recording().envelopes;
    let turn = page
        .turn()
        .map_or_else(|| "-".to_owned(), |turn| turn.to_string());
    let elapsed = match (envelopes.first(), page.clock_ms()) {
        (Some(first), Some(now)) => format!("+{:.1}s", ms_as_secs(now.saturating_sub(first.ts))),
        _ => "+0.0s".to_owned(),
    };
    let link = if extension.is_connected() {
        "connected"
    } else {
        "not connected"
    };
    println!(
        "message {}/{} · turn {turn} · {elapsed} · {link}",
        page.position(),
        envelopes.len()
    );
}

// Recording lengths fit easily in an f64's exact integer range.
#[allow(clippy::cast_precision_loss)]
fn ms_as_secs(ms: u64) -> f64 {
    ms as f64 / 1000.0
}
