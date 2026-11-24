//! The fuzz targets' entries for what this crate parses (cargo-fuzz builds every crate with `--cfg fuzzing`), and
//! the seeds a replay test reads from its repository's `fuzz/seeds`.

use crate::envelope::{Frame, one_way, read_frame, write_frame};
use crate::filled::every_op_filled;
use crate::ops::{Sender, sent_by};
use std::io;
use std::path::{Path, PathBuf};

/// The longest seed kept: a hand seed or a crash's input, never a size a test is slow to read.
pub const MAX_INPUT: usize = 64 * 1024;

/// A server's frame as a client reads it: whatever the bytes, the read ends in an op or a refusal, and an op written
/// back reads as the same op.
pub fn frame_from_server(data: &[u8]) {
    if let Frame::Op(message) = read_frame(data)
        && let Ok(written) = write_frame(&message)
    {
        assert!(matches!(read_frame(&written), Frame::Op(again) if again == message), "an op that does not read back");
    }
}

/// The frames of every op a sender of `target`'s frames may send, filled from the schema and never kept as files: a
/// server's ops for `frame_from_server`, a client's for `frame_signed_in` (an op either end sends is in both), none
/// for any other target. An op too large for a frame is left out.
pub fn made(target: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let senders: &[Sender] = match target {
        "frame_from_server" => &[Sender::Server, Sender::Either],
        "frame_signed_in" => &[Sender::Client, Sender::Either],
        _ => return Vec::new(),
    };
    every_op_filled("secret")
        .into_iter()
        .enumerate()
        .filter(|(_, filled)| senders.contains(&sent_by((&filled.op).into())))
        .filter_map(|(index, filled)| {
            let framed = write_frame(&one_way(1, filled.op)).ok()?;
            Some((PathBuf::from(format!("made/{target}/{}-{index}", filled.name)), framed))
        })
        .collect()
}

/// Every seed for `target`: the ones kept in `folder/<target>` (`folder` is the calling repository's `fuzz/seeds`; a
/// hand seed, or the input of a crash once fixed) by their path, then the ones [`made`]; what the target's replay test
/// runs. A kept one over [`MAX_INPUT`] is refused, and a target with none at all is an error.
pub fn seeds(folder: &Path, target: &str) -> io::Result<Vec<(PathBuf, Vec<u8>)>> {
    let folder = folder.join(target);
    let mut inputs = Vec::new();
    if folder.is_dir() {
        for entry in std::fs::read_dir(&folder)? {
            let path = entry?.path();
            let bytes = std::fs::read(&path)?;
            if bytes.len() > MAX_INPUT {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("over {MAX_INPUT} bytes: {}", path.display()),
                ));
            }
            inputs.push((path, bytes));
        }
    }
    inputs.extend(made(target));
    if inputs.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("no inputs for {target}")));
    }
    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use super::{frame_from_server, made, seeds};
    use crate::chatkord_v1::WireMessage;
    use crate::chatkord_v1::wire_message::Kind;
    use crate::envelope::{Frame, every_op, read_frame};
    use crate::ops::{Sender, sent_by};
    use std::collections::BTreeSet;
    use std::path::Path;

    fn kinds_made(target: &str) -> BTreeSet<&'static str> {
        made(target)
            .iter()
            .filter_map(|(_, bytes)| match read_frame(bytes) {
                Frame::Op(WireMessage { body: Some(op), .. }) => Some(<&'static str>::from(Kind::from(&op))),
                _ => None,
            })
            .collect()
    }

    fn kinds_sent_by(senders: &[Sender]) -> BTreeSet<&'static str> {
        every_op()
            .iter()
            .map(Kind::from)
            .filter(|kind| senders.contains(&sent_by(*kind)))
            .map(<&'static str>::from)
            .collect()
    }

    #[test]
    fn every_op_a_server_sends_has_a_seed() {
        let wanted = kinds_sent_by(&[Sender::Server, Sender::Either]);
        assert!(!wanted.is_empty());
        assert_eq!(kinds_made("frame_from_server"), wanted);
    }

    #[test]
    fn every_op_a_client_sends_has_a_seed() {
        let wanted = kinds_sent_by(&[Sender::Client, Sender::Either]);
        assert!(!wanted.is_empty());
        assert_eq!(kinds_made("frame_signed_in"), wanted);
    }

    #[test]
    fn a_target_no_op_feeds_has_none_made() {
        assert!(made("picture_intake").is_empty());
    }

    #[test]
    fn every_kept_frame_from_a_server_reads_to_an_op_or_a_refusal() {
        let kept = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fuzz/seeds");
        for (_, input) in seeds(&kept, "frame_from_server").expect("the seeds") {
            frame_from_server(&input);
        }
    }
}
