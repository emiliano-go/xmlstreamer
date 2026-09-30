//! `rypipe_core::Splitter` for xmlstreamer feeds.

use rypipe_core::decoder::SkipRegionFinder;
use rypipe_core::Splitter;

use crate::scan;

struct XmlSkipRegions;

impl SkipRegionFinder for XmlSkipRegions {
    fn openers(&self) -> &[&'static [u8]] {
        &[b"<!--", b"<![CDATA[", b"<?"]
    }

    fn closer_for(&self, opener: &[u8]) -> &'static [u8] {
        match opener {
            b"<!--" => b"-->",
            b"<![CDATA[" => b"]]>",
            b"<?" => b"?>",
            _ => unreachable!("unknown opener"),
        }
    }
}

static XML_SKIP_REGIONS: XmlSkipRegions = XmlSkipRegions;

/// Finds separator-tag boundaries, skipping markup sections.
#[derive(Clone)]
pub struct XmlSplitter {
    sep: Vec<u8>,
    open_pattern: Vec<u8>,
}

impl XmlSplitter {
    pub fn new(separator_tag: &str) -> Self {
        let sep = separator_tag.as_bytes().to_vec();
        let mut open_pattern = Vec::with_capacity(sep.len() + 1);
        open_pattern.push(b'<');
        open_pattern.extend_from_slice(&sep);
        Self { sep, open_pattern }
    }
}

impl Splitter for XmlSplitter {
    fn next_record_start(&self, bytes: &[u8], from: usize) -> Option<usize> {
        let mut i = from;
        while i < bytes.len() {
            let (start, after_open) = scan::find_open_sep(bytes, i, &self.sep)?;
            // Self-closing separators carry no content: the next record is
            // the one after them.
            if scan::is_self_closing(&bytes[start..after_open]) {
                i = after_open;
                continue;
            }
            return Some(start);
        }
        None
    }

    fn estimate_bytes_per_row(&self, sample: &[u8]) -> usize {
        let count = memchr::memmem::find_iter(sample, &self.open_pattern)
            .count()
            .max(1);
        (sample.len() / count).max(1)
    }

    fn skip_regions(&self) -> Option<&dyn SkipRegionFinder> {
        Some(&XML_SKIP_REGIONS)
    }
}
