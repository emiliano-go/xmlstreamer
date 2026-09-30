//! Byte-scanning helpers shared by the rypipe `Splitter` and `RecordParser`.
//!
//! These locate the separator tags lexically and skip markup sections
//! (comments, CDATA, processing instructions) so a separator written inside
//! one is never mistaken for a record boundary. DOCTYPE is intentionally not
//! a section here: its quotes and internal-subset brackets make it a walk of
//! its own, and a separator inside a DOCTYPE is not a real-record case the
//! adapter has to support.

/// Python's bytes `\s` set, which is what the scanner's tag patterns use.
#[inline]
pub fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0b' | b'\x0c')
}

/// Return `(terminator, header_len)` when a section opens at `start`.
#[inline]
pub fn section_at(bytes: &[u8], start: usize) -> Option<(&'static [u8], usize)> {
    let rest = bytes.get(start..)?;
    if rest.starts_with(b"<!--") {
        return Some((b"-->", 4));
    }
    if rest.starts_with(b"<![CDATA[") {
        return Some((b"]]>", 9));
    }
    if rest.starts_with(b"<?") {
        return Some((b"?>", 2));
    }
    None
}

#[inline]
pub fn find_seq(bytes: &[u8], from: usize, seq: &[u8]) -> Option<usize> {
    if from > bytes.len() {
        return None;
    }
    memchr::memmem::find(&bytes[from..], seq).map(|r| from + r)
}

/// End (exclusive) of the tag whose content starts at `from` (the byte after
/// `<`): the position just past the `>` that closes it, skipping `>` inside
/// quotes. `None` when it does not close in these bytes.
fn find_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let n = bytes.len();
    let mut i = from;
    while i < n {
        match bytes[i] {
            b'>' => return Some(i + 1),
            b'"' | b'\'' => {
                let close = memchr::memchr(bytes[i], &bytes[i + 1..])?;
                i = i + 1 + close + 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Find the next opening separator tag at or after `from`.
///
/// Returns `(start_of_tag, position_after_the_open_tag)`. Sections are skipped
/// whole; a section that does not close in these bytes stops the search
/// (`None`): nothing after it can be trusted as a record boundary.
pub fn find_open_sep(bytes: &[u8], from: usize, sep: &[u8]) -> Option<(usize, usize)> {
    let n = bytes.len();
    let mut i = from;
    while i < n {
        let lt = memchr::memchr(b'<', &bytes[i..])?;
        let p = i + lt;
        if let Some((terminator, header)) = section_at(bytes, p) {
            match find_seq(bytes, p + header, terminator) {
                Some(end) => {
                    i = end + terminator.len();
                    continue;
                }
                None => return None,
            }
        }
        if p + 1 + sep.len() <= n && &bytes[p + 1..p + 1 + sep.len()] == sep {
            let q = p + 1 + sep.len();
            match bytes.get(q) {
                Some(b'>') => return Some((p, q + 1)),
                Some(&b) if is_ws(b) => return find_tag_end(bytes, q).map(|end| (p, end)),
                _ => {}
            }
        }
        i = p + 1;
    }
    None
}

/// Find the next `</sep>` at or after `from`, skipping sections.
///
/// Returns `(start_of_close_tag, position_after_it)`.
pub fn find_close_sep(bytes: &[u8], from: usize, sep: &[u8]) -> Option<(usize, usize)> {
    let n = bytes.len();
    let mut i = from;
    while i < n {
        let lt = memchr::memchr(b'<', &bytes[i..])?;
        let p = i + lt;
        if let Some((terminator, header)) = section_at(bytes, p) {
            match find_seq(bytes, p + header, terminator) {
                Some(end) => {
                    i = end + terminator.len();
                    continue;
                }
                None => return None,
            }
        }
        if p + 2 + sep.len() <= n
            && bytes[p + 1] == b'/'
            && &bytes[p + 2..p + 2 + sep.len()] == sep
        {
            let q = p + 2 + sep.len();
            if let Some(r) = memchr::memchr(b'>', &bytes[q..]) {
                return Some((p, q + r + 1));
            }
            return None;
        }
        i = p + 1;
    }
    None
}

/// Whether an opening separator tag (`<sep ...>`) is self-closing (`<sep/>`).
#[inline]
pub fn is_self_closing(open_tag: &[u8]) -> bool {
    let mut end = open_tag.len().saturating_sub(1); // the closing ">"
    while end > 0 && is_ws(open_tag[end - 1]) {
        end -= 1;
    }
    end > 0 && open_tag[end - 1] == b'/'
}
