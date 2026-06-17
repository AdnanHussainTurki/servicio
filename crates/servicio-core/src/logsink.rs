use crate::error::CoreError;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Appends tagged log lines to a file, rotating by size. Synchronous and simple:
/// the supervisor calls it from a blocking-friendly context per line.
pub struct LogSink {
    path: PathBuf,
    file: File,
    written: u64,
    max_bytes: u64,
    max_files: u32,
}

impl LogSink {
    pub fn new(path: &Path, max_bytes: u64, max_files: u32) -> Result<Self, CoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let written = file.metadata()?.len();
        Ok(Self { path: path.to_path_buf(), file, written, max_bytes, max_files })
    }

    /// Write one line tagged with instance index + stream name + a timestamp marker.
    pub fn write_line(&mut self, instance: u32, stream: &str, line: &str) -> Result<(), CoreError> {
        let record = format!("[#{instance}] [{stream}] {line}\n");
        if self.written + record.len() as u64 > self.max_bytes {
            self.rotate()?;
        }
        self.file.write_all(record.as_bytes())?;
        self.written += record.len() as u64;
        Ok(())
    }

    /// Shift w.log -> w.log.1 -> w.log.2 ... dropping anything past max_files.
    fn rotate(&mut self) -> Result<(), CoreError> {
        for i in (1..self.max_files).rev() {
            let from = self.indexed(i);
            let to = self.indexed(i + 1);
            if from.exists() {
                std::fs::rename(&from, &to)?;
            }
        }
        std::fs::rename(&self.path, self.indexed(1))?;
        self.file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        self.written = 0;
        Ok(())
    }

    fn indexed(&self, i: u32) -> PathBuf {
        self.path.with_extension(format!("log.{i}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn writes_tagged_lines_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.log");
        let mut sink = LogSink::new(&path, 1_000_000, 3).unwrap();
        sink.write_line(2, "stdout", "processing job").unwrap();

        let mut contents = String::new();
        File::open(&path).unwrap().read_to_string(&mut contents).unwrap();
        assert!(contents.contains("[#2]"));
        assert!(contents.contains("stdout"));
        assert!(contents.contains("processing job"));
    }

    #[test]
    fn rotates_when_size_cap_exceeded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.log");
        // tiny cap forces rotation after the first line.
        let mut sink = LogSink::new(&path, 10, 3).unwrap();
        sink.write_line(1, "stdout", "first line is long enough").unwrap();
        sink.write_line(1, "stdout", "second").unwrap();

        // rotated file w.log.1 must now exist.
        assert!(path.with_extension("log.1").exists());
    }
}
