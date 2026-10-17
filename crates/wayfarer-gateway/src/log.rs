//! A session's output log: every chunk on disk (`output.log`, JSON lines) and the most recent
//! ones in memory, answering history requests from either.
//!
//! One record per line: `{"seq":1,"ts":1760000000000,"text":"..."}` (`ts` in milliseconds since
//! the Unix epoch). JSON escapes every newline in the text, so a record is always one line;
//! start-up reads only the file's tail to find the last `seq`, and a disk lookup is a binary
//! search on byte offsets, since `seq` rises through the file.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use wayfarer_proto::Chunk;

/// Which chunks a history request wants (the wire's `history_request`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryQuery {
    /// The chunks after this `seq`, oldest first.
    pub after_seq: Option<u64>,
    /// The chunks just before this `seq`.
    pub before_seq: Option<u64>,
    pub max_chunks: u32,
}

impl HistoryQuery {
    /// The latest `max_chunks` chunks.
    pub fn latest(max_chunks: u32) -> Self {
        Self {
            after_seq: None,
            before_seq: None,
            max_chunks,
        }
    }

    pub fn after(seq: u64, max_chunks: u32) -> Self {
        Self {
            after_seq: Some(seq),
            ..Self::latest(max_chunks)
        }
    }

    pub fn before(seq: u64, max_chunks: u32) -> Self {
        Self {
            before_seq: Some(seq),
            ..Self::latest(max_chunks)
        }
    }

    /// The `seq` range `lo..=hi` this asks for when the latest chunk is `last` (empty when
    /// `lo > hi`). With `after_seq` the range starts just after it; otherwise it ends just
    /// before `before_seq` (or at `last`).
    fn range(&self, last: u64) -> (u64, u64) {
        let max = u64::from(self.max_chunks);
        let hi = self
            .before_seq
            .map_or(last, |b| b.saturating_sub(1))
            .min(last);
        match self.after_seq {
            Some(a) => {
                let lo = a.saturating_add(1);
                (lo, hi.min(a.saturating_add(max)))
            }
            None => ((hi + 1).saturating_sub(max).max(1), hi),
        }
    }
}

/// The answer to a [`HistoryQuery`] (the wire's `history`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// The first chunk's `seq`, or where the range would have started if `chunks` is empty.
    pub from_seq: u64,
    /// Contiguous, in ascending `seq`.
    pub chunks: Vec<Chunk>,
}

#[derive(Serialize)]
struct RecordOut<'a> {
    seq: u64,
    ts: u64,
    text: &'a str,
}

#[derive(Deserialize)]
struct Record {
    seq: u64,
    text: String,
}

pub(crate) struct OutputLog {
    path: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    file: File,
    last_seq: u64,
    /// The most recent chunks, oldest first, contiguous and ending at `last_seq`.
    recent: VecDeque<Chunk>,
    recent_bytes: usize,
    memory_limit: usize,
    /// A write failed; warn once until one succeeds again.
    failing: bool,
}

impl OutputLog {
    /// Open (creating) `dir/output.log`, continuing from its last record. A record cut short by
    /// a crash is dropped.
    pub fn open(dir: &Path, memory_limit: usize) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("output.log");
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)?;
        let last_seq = recover(&mut file, &path)?;
        Ok(Self {
            path,
            inner: Mutex::new(Inner {
                file,
                last_seq,
                recent: VecDeque::new(),
                recent_bytes: 0,
                memory_limit,
                failing: false,
            }),
        })
    }

    pub fn last_seq(&self) -> u64 {
        self.lock().last_seq
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Nothing panics while holding the lock, but don't lose the log if something did.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Give `text` the next `seq`, write it to disk and keep it in memory. A failed disk write
    /// is reported (once until writes work again) and does not stop the session: the chunk
    /// still gets its `seq` and stays in memory.
    pub fn append(&self, text: String) -> Chunk {
        let mut inner = self.lock();
        let seq = inner.last_seq + 1;
        let mut line = serde_json::to_vec(&RecordOut {
            seq,
            ts: now_ms(),
            text: &text,
        })
        .expect("a record always serializes");
        line.push(b'\n');
        match inner.file.write_all(&line) {
            Ok(()) => inner.failing = false,
            Err(e) => {
                if !inner.failing {
                    tracing::warn!("{}: write failed: {e}", self.path.display());
                }
                inner.failing = true;
            }
        }
        inner.last_seq = seq;
        let chunk = Chunk { seq, text };
        inner.recent_bytes += chunk.text.len();
        inner.recent.push_back(chunk.clone());
        while inner.recent_bytes > inner.memory_limit && inner.recent.len() > 1 {
            let old = inner.recent.pop_front().expect("not empty");
            inner.recent_bytes -= old.text.len();
        }
        chunk
    }

    /// Answer a history query: from memory where it can, the rest from disk. Reading the disk
    /// blocks, so call this from a blocking context.
    pub fn history(&self, query: HistoryQuery) -> io::Result<History> {
        let (lo, hi, from_memory, memory_start) = {
            let inner = self.lock();
            let (lo, hi) = query.range(inner.last_seq);
            let memory_start = inner.recent.front().map_or(inner.last_seq + 1, |c| c.seq);
            let from_memory: Vec<Chunk> = if lo > hi {
                Vec::new()
            } else {
                let start = inner.recent.partition_point(|c| c.seq < lo);
                inner
                    .recent
                    .range(start..)
                    .take_while(|c| c.seq <= hi)
                    .cloned()
                    .collect()
            };
            (lo, hi, from_memory, memory_start)
        };
        // Everything before `memory_start` was written to disk before it left memory.
        let mut chunks = if lo < memory_start && lo <= hi {
            read_disk(&self.path, lo, hi.min(memory_start - 1))?
        } else {
            Vec::new()
        };
        chunks.extend(from_memory);
        Ok(History {
            from_seq: lo,
            chunks,
        })
    }

    /// How many chunks are in memory (tests check the disk path is used).
    #[cfg(test)]
    fn in_memory(&self) -> usize {
        self.lock().recent.len()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// The last record's `seq` (0 for an empty file), cutting off a partial last line.
fn recover(file: &mut File, path: &Path) -> io::Result<u64> {
    let len = file.metadata()?.len();
    let mut take = 4096u64;
    loop {
        let start = len.saturating_sub(take);
        let mut tail = vec![0; (len - start) as usize];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut tail)?;
        let Some(end) = tail.iter().rposition(|&b| b == b'\n') else {
            if start == 0 {
                truncate(file, path, 0, len)?;
                return Ok(0);
            }
            take *= 2;
            continue;
        };
        let line_start = match tail[..end].iter().rposition(|&b| b == b'\n') {
            Some(i) => i + 1,
            None if start == 0 => 0,
            None => {
                take *= 2;
                continue;
            }
        };
        let record: Record = serde_json::from_slice(&tail[line_start..end]).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: last record unreadable: {e}", path.display()),
            )
        })?;
        truncate(file, path, start + end as u64 + 1, len)?;
        return Ok(record.seq);
    }
}

fn truncate(file: &File, path: &Path, keep: u64, len: u64) -> io::Result<()> {
    if keep < len {
        tracing::warn!(
            "{}: dropping {} bytes of an unfinished last record",
            path.display(),
            len - keep
        );
        file.set_len(keep)?;
    }
    Ok(())
}

/// The records with `seq` in `lo..=hi` from the file.
fn read_disk(path: &Path, lo: u64, hi: u64) -> io::Result<Vec<Chunk>> {
    let mut reader = BufReader::new(File::open(path)?);
    let len = reader.get_ref().metadata()?.len();
    // The smallest offset whose next record has seq >= lo.
    let (mut a, mut b) = (0u64, len);
    while a < b {
        let mid = a + (b - a) / 2;
        match record_from(&mut reader, mid, len)? {
            Some((_, seq)) if seq < lo => a = mid + 1,
            _ => b = mid,
        }
    }
    let mut out = Vec::new();
    seek_line(&mut reader, a)?;
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 || line.last() != Some(&b'\n') {
            break;
        }
        let Ok(record) = serde_json::from_slice::<Record>(&line) else {
            continue;
        };
        if record.seq > hi {
            break;
        }
        if record.seq >= lo {
            out.push(Chunk {
                seq: record.seq,
                text: record.text,
            });
        }
    }
    Ok(out)
}

/// Position `reader` at the first line starting at or after `pos`.
fn seek_line(reader: &mut BufReader<File>, pos: u64) -> io::Result<u64> {
    if pos == 0 {
        reader.seek(SeekFrom::Start(0))?;
        return Ok(0);
    }
    reader.seek(SeekFrom::Start(pos - 1))?;
    let skipped = reader.skip_until(b'\n')?;
    Ok(pos - 1 + skipped as u64)
}

/// The first readable record starting at or after `pos` (its offset and `seq`), or `None` at
/// the end of the file.
fn record_from(reader: &mut BufReader<File>, pos: u64, len: u64) -> io::Result<Option<(u64, u64)>> {
    let mut at = seek_line(reader, pos)?;
    let mut line = Vec::new();
    while at < len {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 || line.last() != Some(&b'\n') {
            break;
        }
        if let Ok(record) = serde_json::from_slice::<Record>(&line) {
            return Ok(Some((at, record.seq)));
        }
        at += n as u64;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(chunks: &[Chunk]) -> Vec<(u64, &str)> {
        chunks.iter().map(|c| (c.seq, c.text.as_str())).collect()
    }

    /// The chunks `lo..=hi` of `all` (seq n is all[n-1]).
    fn expected(all: &[Chunk], lo: u64, hi: u64) -> Vec<Chunk> {
        all.iter()
            .filter(|c| c.seq >= lo && c.seq <= hi)
            .cloned()
            .collect()
    }

    #[test]
    fn ranges() {
        let r = |q: HistoryQuery| q.range(10);
        assert_eq!(r(HistoryQuery::after(0, 3)), (1, 3));
        assert_eq!(r(HistoryQuery::after(8, 5)), (9, 10));
        assert_eq!(r(HistoryQuery::after(10, 5)), (11, 10));
        assert_eq!(r(HistoryQuery::after(50, 5)), (51, 10));
        assert_eq!(r(HistoryQuery::before(5, 2)), (3, 4));
        assert_eq!(r(HistoryQuery::before(3, 9)), (1, 2));
        assert_eq!(r(HistoryQuery::before(1, 9)), (1, 0));
        assert_eq!(r(HistoryQuery::before(99, 3)), (8, 10));
        assert_eq!(r(HistoryQuery::latest(4)), (7, 10));
        assert_eq!(r(HistoryQuery::latest(0)), (11, 10));
        let both = HistoryQuery {
            after_seq: Some(2),
            before_seq: Some(6),
            max_chunks: 9,
        };
        assert_eq!(r(both), (3, 5));
        assert_eq!(HistoryQuery::latest(5).range(0), (1, 0));
    }

    #[test]
    fn every_query_across_the_memory_disk_boundary() {
        let dir = tempfile::tempdir().unwrap();
        // 40 bytes of memory: the last four or five chunks.
        let log = OutputLog::open(dir.path(), 40).unwrap();
        let mut all = Vec::new();
        for i in 1..=60u64 {
            // Newlines, quotes and non-ASCII must survive the JSON line.
            let text = format!("l{i}\r\n\"█\"{}", "x".repeat((i % 7) as usize));
            let chunk = log.append(text);
            assert_eq!(chunk.seq, i);
            all.push(chunk);
        }
        assert!(log.in_memory() < 10, "{}", log.in_memory());
        for max in [0u32, 1, 3, 7, 100] {
            for seq in 0..=63u64 {
                let q = HistoryQuery::after(seq, max);
                let (lo, hi) = q.range(60);
                let h = log.history(q).unwrap();
                assert_eq!(h.from_seq, seq + 1);
                assert_eq!(h.chunks, expected(&all, lo, hi), "after {seq} max {max}");
                let q = HistoryQuery::before(seq, max);
                let (lo, hi) = q.range(60);
                let h = log.history(q).unwrap();
                assert_eq!(h.from_seq, lo);
                assert_eq!(h.chunks, expected(&all, lo, hi), "before {seq} max {max}");
            }
        }
        let h = log.history(HistoryQuery::latest(100)).unwrap();
        assert_eq!((h.from_seq, h.chunks), (1, all.clone()));

        // Reopened, everything comes from disk and seq continues.
        drop(log);
        let log = OutputLog::open(dir.path(), 40).unwrap();
        assert_eq!(log.last_seq(), 60);
        assert_eq!(log.in_memory(), 0);
        assert_eq!(
            log.history(HistoryQuery::after(55, 3)).unwrap().chunks,
            all[55..58]
        );
        assert_eq!(log.append("next".into()).seq, 61);
    }

    #[test]
    fn a_partial_last_record_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let log = OutputLog::open(dir.path(), 1 << 20).unwrap();
        log.append("one".into());
        log.append("two".into());
        drop(log);
        let path = dir.path().join("output.log");
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(br#"{"seq":3,"ts":1,"te"#).unwrap();
        drop(f);
        let log = OutputLog::open(dir.path(), 1 << 20).unwrap();
        assert_eq!(log.last_seq(), 2);
        assert_eq!(log.append("three".into()).seq, 3);
        let h = log.history(HistoryQuery::after(0, 10)).unwrap();
        assert_eq!(
            texts(&h.chunks),
            [(1, "one"), (2, "two"), (3, "three")],
            "{:?}",
            std::fs::read_to_string(&path)
        );
    }

    #[test]
    fn long_records_and_an_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let log = OutputLog::open(dir.path(), 0).unwrap();
        assert_eq!(log.last_seq(), 0);
        assert_eq!(
            log.history(HistoryQuery::latest(10)).unwrap(),
            History {
                from_seq: 1,
                chunks: vec![]
            }
        );
        // Longer than the 4 KiB first read at start-up.
        let big = "y".repeat(10_000);
        log.append(big.clone());
        log.append(big.clone());
        drop(log);
        let log = OutputLog::open(dir.path(), 0).unwrap();
        assert_eq!(log.last_seq(), 2);
        let h = log.history(HistoryQuery::before(3, 1)).unwrap();
        assert_eq!(texts(&h.chunks), [(2, big.as_str())]);
    }

    #[test]
    fn a_single_oversized_chunk_stays_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let log = OutputLog::open(dir.path(), 4).unwrap();
        log.append("0123456789".into());
        assert_eq!(log.in_memory(), 1);
        log.append("abcdefghij".into());
        assert_eq!(log.in_memory(), 1);
    }
}
