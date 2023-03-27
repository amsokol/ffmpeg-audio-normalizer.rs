use std::io::{stderr, stdout, BufRead, Write};

pub fn to_stdout<R: BufRead>(reader: R) {
    let stdout: std::io::Stdout = stdout();
    let mut lock = stdout.lock();

    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| {
            let _ = writeln!(lock, "{line}");
        });
}

pub fn to_stderr<R: BufRead>(reader: R) {
    let stderr = stderr();
    let mut lock = stderr.lock();

    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| {
            let _ = writeln!(lock, "{line}");
        });
}
