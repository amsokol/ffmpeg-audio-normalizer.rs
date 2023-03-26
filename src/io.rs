use std::io;
use std::io::Write;

pub fn to_stdout<R: io::BufRead>(reader: R) {
    let stdout = io::stdout();
    let mut lock = stdout.lock();

    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| {
            let _ = writeln!(lock, "{line}");
        });
}

pub fn to_stderr<R: io::BufRead>(reader: R) {
    let stderr = io::stderr();
    let mut lock = stderr.lock();

    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| {
            let _ = writeln!(lock, "{line}");
        });
}
