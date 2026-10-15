//! The Recording file format: written while a game is played, read back for replay.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_replay::{Recording, RecordingWriter};
use copilot_webtiles::Envelope;

const COMMITTED: &[u8] = include_bytes!("recordings/minotaur-first-steps.dcssrec");

fn envelopes() -> Vec<Envelope> {
    Recording::read(COMMITTED).unwrap().envelopes
}

#[test]
fn a_finished_recording_reads_back_every_envelope_in_order() {
    let mut writer = RecordingWriter::new(Vec::new(), "1.2.3").unwrap();
    for envelope in &envelopes() {
        writer.write(envelope).unwrap();
    }

    let recording = Recording::read(&writer.finish().unwrap()[..]).unwrap();

    assert_eq!(recording.header.app_version, "1.2.3");
    assert_eq!(recording.envelopes, envelopes());
    assert!(!recording.truncated);
}

#[test]
fn a_recording_cut_short_by_a_crash_reads_up_to_its_last_flush_and_says_so() {
    let mut file = Vec::new();
    let mut writer = RecordingWriter::new(&mut file, "1.2.3").unwrap();
    let all = envelopes();
    for envelope in &all[..4] {
        writer.write(envelope).unwrap();
    }
    writer.flush().unwrap();
    writer.write(&all[4]).unwrap();
    // The app dies here: the writer is never finished.
    std::mem::forget(writer);

    let recording = Recording::read(&file[..]).unwrap();

    assert_eq!(recording.envelopes, all[..4]);
    assert!(recording.truncated);
}

#[test]
fn a_file_that_is_not_a_recording_is_refused() {
    let compressed = zstd::encode_all(
        &b"{\"format\":\"something-else\",\"formatVersion\":1,\"appVersion\":\"1\"}\n"[..],
        3,
    )
    .unwrap();

    let error = Recording::read(&compressed[..]).unwrap_err();

    assert!(error.to_string().contains("something-else"), "{error}");
}

#[test]
fn a_recording_is_zstd_json_lines_a_header_then_the_envelopes_as_relayed() {
    let committed = String::from_utf8(zstd::decode_all(COMMITTED).unwrap()).unwrap();
    let mut lines = committed.lines();
    assert_eq!(
        lines.next(),
        Some(r#"{"format":"dcssrec","formatVersion":1,"appVersion":"0.0.0-test"}"#)
    );
    assert!(lines.next().unwrap().starts_with(
        r#"{"origin":"https://crawl.example.org","tab":7,"ts":1760486400000,"gameVersion":null,"msg":{"msg":"login_success","#
    ));

    let mut writer = RecordingWriter::new(Vec::new(), "0.0.0-test").unwrap();
    for envelope in &envelopes() {
        writer.write(envelope).unwrap();
    }
    let rewritten =
        String::from_utf8(zstd::decode_all(&writer.finish().unwrap()[..]).unwrap()).unwrap();

    assert_eq!(rewritten, committed);
}
