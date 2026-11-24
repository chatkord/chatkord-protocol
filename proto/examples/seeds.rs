//! Writes the seeds made from the schema for a fuzz target (`proto::fuzz::made`) into a folder: `seeds <target> <dir>`.

#![expect(clippy::print_stderr, reason = "a command line tool's refusal is its standard error")]

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let (Some(target), Some(folder), None) = (arguments.next(), arguments.next(), arguments.next()) else {
        eprintln!("usage: seeds <target> <dir>");
        return ExitCode::from(2);
    };
    let folder = Path::new(&folder);
    if let Err(failed) = std::fs::create_dir_all(folder) {
        eprintln!("seeds: {}: {failed}", folder.display());
        return ExitCode::FAILURE;
    }
    for (path, bytes) in proto::fuzz::made(&target) {
        let Some(name) = path.file_name() else { continue };
        if let Err(failed) = std::fs::write(folder.join(name), bytes) {
            eprintln!("seeds: {}: {failed}", name.to_string_lossy());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
