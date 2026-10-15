//! The Recording file format, `*.dcssrec`: zstd-compressed JSON Lines, a header line then the raw
//! relay envelopes in the order they arrived.

use std::io::{BufRead, BufReader, Read, Write};

use copilot_webtiles::Envelope;
use serde::{Deserialize, Serialize};

/// The file extension Recordings are saved with.
pub const RECORDING_EXTENSION: &str = "dcssrec";

/// The header's `format`, so a stray file is recognised as not a Recording.
const FORMAT_TAG: &str = "dcssrec";

/// Bumped whenever the header or line format changes.
const FORMAT_VERSION: u32 = 1;

/// A balance of speed and size for a file written while the game is played.
const ZSTD_LEVEL: i32 = 3;

/// The first line of every Recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingHeader {
    /// Always `dcssrec`, so a stray file is recognised as not a Recording.
    pub format: String,
    /// The Recording format version.
    pub format_version: u32,
    /// The version of the app that wrote it.
    pub app_version: String,
}

/// Why a Recording couldn't be written or read.
#[derive(Debug, thiserror::Error)]
pub enum RecordingError {
    /// The file couldn't be written, read or decompressed.
    #[error("recording IO failed: {0}")]
    Io(#[from] std::io::Error),
    /// A line isn't the JSON it should be.
    #[error("recording line {line} is malformed: {source}")]
    Malformed {
        /// The 1-based line number.
        line: usize,
        /// What was wrong with it.
        source: serde_json::Error,
    },
    /// The file has no header line.
    #[error("recording is empty")]
    Empty,
    /// The header names a format this build can't read.
    #[error("not a recording this build can read: format {format:?} version {version}")]
    UnsupportedFormat {
        /// The header's `format`.
        format: String,
        /// The header's `formatVersion`.
        version: u32,
    },
}

/// Writes a Recording, one envelope at a time.
pub struct RecordingWriter<W: Write> {
    encoder: zstd::Encoder<'static, W>,
}

impl<W: Write> RecordingWriter<W> {
    /// Starts a Recording by writing its header.
    ///
    /// # Errors
    /// If the header can't be written.
    pub fn new(writer: W, app_version: &str) -> Result<Self, RecordingError> {
        let mut recording = Self {
            encoder: zstd::Encoder::new(writer, ZSTD_LEVEL)?,
        };
        let header = RecordingHeader {
            format: FORMAT_TAG.to_owned(),
            format_version: FORMAT_VERSION,
            app_version: app_version.to_owned(),
        };
        recording.write_line(&header)?;
        Ok(recording)
    }

    /// Appends one envelope.
    ///
    /// # Errors
    /// If the envelope can't be written.
    pub fn write(&mut self, envelope: &Envelope) -> Result<(), RecordingError> {
        self.write_line(envelope)
    }

    /// Pushes everything written so far to the underlying writer, so a Recording cut short by a
    /// crash can still be read up to here.
    ///
    /// # Errors
    /// If the flush fails.
    pub fn flush(&mut self) -> Result<(), RecordingError> {
        Ok(self.encoder.flush()?)
    }

    /// Ends the Recording, so it can be read to the last envelope, and returns the writer.
    ///
    /// # Errors
    /// If the end of the compressed stream can't be written.
    pub fn finish(self) -> Result<W, RecordingError> {
        Ok(self.encoder.finish()?)
    }

    fn write_line(&mut self, value: &impl Serialize) -> Result<(), RecordingError> {
        let line = serde_json::to_vec(value).map_err(std::io::Error::other)?;
        self.encoder.write_all(&line)?;
        self.encoder.write_all(b"\n")?;
        Ok(())
    }
}

/// A Recording read back: its header and every envelope, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    /// The header line.
    pub header: RecordingHeader,
    /// The relayed envelopes.
    pub envelopes: Vec<Envelope>,
    /// The file ends early, as when the app crashed before finishing it: `envelopes` holds what
    /// was written up to the last flush.
    pub truncated: bool,
}

impl Recording {
    /// Reads a whole Recording, or as much as was flushed of one cut short.
    ///
    /// # Errors
    /// If it can't be decompressed, a line is malformed, or the format is unsupported.
    pub fn read(reader: impl Read) -> Result<Self, RecordingError> {
        let (lines, truncated) = read_lines(reader)?;
        let (header_line, envelope_lines) = lines.split_first().ok_or(RecordingError::Empty)?;
        let header: RecordingHeader = parse(header_line, 1)?;
        if header.format != FORMAT_TAG || header.format_version != FORMAT_VERSION {
            return Err(RecordingError::UnsupportedFormat {
                format: header.format,
                version: header.format_version,
            });
        }
        let mut envelopes = Vec::with_capacity(envelope_lines.len());
        for (index, line) in envelope_lines.iter().enumerate() {
            let is_last = index + 1 == envelope_lines.len();
            match parse(line, index + 2) {
                Ok(envelope) => envelopes.push(envelope),
                // A file cut short can end part-way through a line.
                Err(_) if truncated && is_last => break,
                Err(error) => return Err(error),
            }
        }
        Ok(Self {
            header,
            envelopes,
            truncated,
        })
    }
}

/// Every decompressed line, and whether the stream ended before its end mark.
fn read_lines(reader: impl Read) -> Result<(Vec<String>, bool), RecordingError> {
    let mut lines = Vec::new();
    for line in BufReader::new(zstd::Decoder::new(reader)?).lines() {
        match line {
            Ok(line) => lines.push(line),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok((lines, true));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok((lines, false))
}

fn parse<T: for<'de> Deserialize<'de>>(line: &str, number: usize) -> Result<T, RecordingError> {
    serde_json::from_str(line).map_err(|source| RecordingError::Malformed {
        line: number,
        source,
    })
}
