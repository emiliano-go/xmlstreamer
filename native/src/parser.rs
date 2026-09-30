//! `rypipe_core::RecordParser` for xmlstreamer feeds.
//!
//! A chunk is aligned to record starts by `XmlSplitter`; this walks the
//! complete items inside it, flattens each into the same `{path: text}`
//! columns the streaming parser produces, and skips items that do not parse.
//! A trailing partial item is left alone: the engine discards it.

use rypipe_core::{ColumnarSink, RecordParser, Value};

use crate::scan;
use crate::xml;

#[derive(Clone)]
pub struct XmlParser {
    sep: Vec<u8>,
}

impl XmlParser {
    pub fn new(separator_tag: &str) -> Self {
        Self {
            sep: separator_tag.as_bytes().to_vec(),
        }
    }

    fn parse_into<S: ColumnarSink + ?Sized>(&self, bytes: &[u8], sink: &mut S) {
        let sep = std::str::from_utf8(&self.sep).unwrap_or("item");
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

            let Some(fields) = xml::parse_item_flat(content, sep) else {
                // A malformed item is dropped alone; its neighbours stand.
                continue;
            };
            if fields.is_empty() {
                continue;
            }
            if fields.len() == 1
                && fields[0].0 == sep
                && fields[0].1.trim_is_empty()
            {
                continue;
            }

            sink.begin_row();
            // Ordinal counts *kept* fields, matching the engine's
            // `current_ordinal` (it does not advance for dropped fields).
            let mut ordinal: u32 = 0;
            for (name, value) in fields {
                if !sink.wants(&name) {
                    continue;
                }
                // Layout prediction: after row 1 the engine caches an
                // ordinal -> (slot, name) map. A memcmp against the raw name
                // lets us skip name resolution and a hash lookup entirely.
                let matched = sink
                    .expect_slot(ordinal)
                    .map(|(slot, expected)| (slot, name.as_bytes() == expected));
                match matched {
                    Some((slot, true)) => {
                        sink.put_field_at(slot, Value::Str(value.rstrip_newlines()));
                    }
                    Some((_, false)) => {
                        sink.layout_broken(ordinal);
                        sink.put_field(&name, Value::Str(value.rstrip_newlines()));
                    }
                    None => {
                        sink.put_field(&name, Value::Str(value.rstrip_newlines()));
                    }
                }
                ordinal += 1;
            }
            sink.end_row();
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
