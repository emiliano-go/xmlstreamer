//! `rypipe_core::RecordParser` for xmlstreamer feeds.
//!
//! A chunk is aligned to record starts by `XmlSplitter`; this walks the
//! complete items inside it, flattens each into the same `{path: text}`
//! columns the streaming parser produces, and skips items that do not parse.
//! A trailing partial item is left alone: the engine discards it.

use std::borrow::Cow;
use std::cell::RefCell;

use rypipe_core::{ColumnarSink, RecordParser, Value};

use crate::scan;
use crate::xml::{self, FlatEmitter, Scratch};

// One reusable parse scratch per OS thread. The engine shares a single parser
// across rayon workers (and clones it for streaming), so a parser-owned
// `Mutex<Scratch>` would serialize every worker; a thread-local keeps the
// buffers per worker with no contention.
thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

#[derive(Clone)]
pub struct XmlParser {
    sep: Vec<u8>,
}

/// Emits one row per item, using the engine's layout-prediction fast path.
struct RowEmitter<'a, S: ColumnarSink + ?Sized> {
    sink: &'a mut S,
    ordinal: u32,
}

impl<S: ColumnarSink + ?Sized> FlatEmitter for RowEmitter<'_, S> {
    fn row_start(&mut self) {
        self.sink.begin_row();
        self.ordinal = 0;
    }

    fn field(&mut self, name: &str, value: &str) {
        if !self.sink.wants(name) {
            return;
        }
        let matched = self
            .sink
            .expect_slot(self.ordinal)
            .map(|(slot, expected)| (slot, name.as_bytes() == expected));
        match matched {
            Some((slot, true)) => {
                self.sink.put_field_at(slot, Value::Str(Cow::Borrowed(value)));
            }
            Some((_, false)) => {
                self.sink.layout_broken(self.ordinal);
                self.sink.put_field(name, Value::Str(Cow::Borrowed(value)));
            }
            None => {
                self.sink.put_field(name, Value::Str(Cow::Borrowed(value)));
            }
        }
        self.ordinal += 1;
    }

    fn row_end(&mut self) {
        self.sink.end_row();
    }

    fn wants(&self, name: &str) -> bool {
        self.sink.wants(name)
    }
}

impl XmlParser {
    pub fn new(separator_tag: &str) -> Self {
        Self {
            sep: separator_tag.as_bytes().to_vec(),
        }
    }

    fn parse_into<S: ColumnarSink + ?Sized>(&self, bytes: &[u8], sink: &mut S) {
        let sep = std::str::from_utf8(&self.sep).unwrap_or("item");
        SCRATCH.with(|cell| {
            let mut scratch = cell.borrow_mut();
            self.scan_and_emit(bytes, sep, &mut scratch, sink);
        });
    }

    fn scan_and_emit<S: ColumnarSink + ?Sized>(
        &self,
        bytes: &[u8],
        sep: &str,
        scratch: &mut Scratch,
        sink: &mut S,
    ) {
        let mut i = 0usize;
        loop {
            let Some((start, after_open)) = scan::find_open_sep(bytes, i, &self.sep) else {
                break;
            };
            if scan::is_self_closing(&bytes[start..after_open]) {
                i = after_open;
                continue;
            }
            let Some((close_start, after_close)) =
                scan::find_close_sep(bytes, after_open, &self.sep)
            else {
                // The item is incomplete in this chunk: the engine drops the
                // trailing partial row.
                break;
            };
            let content = &bytes[after_open..close_start];
            i = after_close;

            if content.is_empty() || content.iter().all(|&b| scan::is_ws(b)) {
                continue;
            }

            let mut emitter = RowEmitter {
                sink: &mut *sink,
                ordinal: 0,
            };
            // A malformed item returns None and emits nothing (dropped whole).
            let _ = xml::parse_item_flat_with(scratch, content, sep, &mut emitter);
        }
    }
}

impl RecordParser for XmlParser {
    fn validate(&self, bytes: &[u8]) -> rypipe_core::Result<()> {
        simdutf8::basic::from_utf8(bytes).map_err(rypipe_core::Error::Utf8)?;
        Ok(())
    }

    fn parse_chunk(&self, bytes: &[u8], sink: &mut dyn ColumnarSink) -> rypipe_core::Result<()> {
        self.parse_into(bytes, sink);
        Ok(())
    }

    fn parse_chunk_generic<S: ColumnarSink>(
        &self,
        bytes: &[u8],
        sink: &mut S,
    ) -> rypipe_core::Result<()> {
        self.parse_into(bytes, sink);
        Ok(())
    }
}
