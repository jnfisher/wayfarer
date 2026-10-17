//! Raw capture: everything received (before telnet stripping) and everything sent, each in
//! its own file, flushed per chunk so a crash loses nothing.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub struct Capture {
    received: File,
    sent: File,
}

impl Capture {
    /// Open (creating, appending to) the two files.
    pub fn create(received: impl AsRef<Path>, sent: impl AsRef<Path>) -> io::Result<Self> {
        let open = |p: &Path| OpenOptions::new().create(true).append(true).open(p);
        Ok(Self {
            received: open(received.as_ref())?,
            sent: open(sent.as_ref())?,
        })
    }

    pub fn record_received(&mut self, bytes: &[u8]) -> io::Result<()> {
        write_flushed(&mut self.received, bytes)
    }

    pub fn record_sent(&mut self, bytes: &[u8]) -> io::Result<()> {
        write_flushed(&mut self.sent, bytes)
    }
}

fn write_flushed(f: &mut File, bytes: &[u8]) -> io::Result<()> {
    f.write_all(bytes)?;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_both_files() {
        let dir = std::env::temp_dir().join(format!("wayfarer-telnet-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (r, s) = (dir.join("raw.bin"), dir.join("sent.bin"));
        let _ = std::fs::remove_file(&r);
        let _ = std::fs::remove_file(&s);
        let mut c = Capture::create(&r, &s).unwrap();
        c.record_received(&[255, 251, 1]).unwrap();
        c.record_received(b"hi").unwrap();
        c.record_sent(b"x\r").unwrap();
        // Readable while the capture is still open: nothing is held back.
        assert_eq!(std::fs::read(&r).unwrap(), [255, 251, 1, b'h', b'i']);
        assert_eq!(std::fs::read(&s).unwrap(), b"x\r");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
