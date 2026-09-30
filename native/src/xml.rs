//! XML name/char rules and the per-item document parser.
//!
//! These are a Rust port of the rules xmlstreamer previously delegated to
//! expat and of `ItemHandler`: a wrapped item is parsed in isolation into a
//! flat insertion-ordered `{path: text}` map, and anything not well formed
//! invalidates the item alone.

const XML_WHITESPACE: [u8; 4] = [b' ', b'\t', b'\r', b'\n'];
const NAME_STOP: [u8; 10] = [
    b' ', b'\t', b'\r', b'\n', b'=', b'/', b'<', b'>', b'"', b'\'',
];

// ---------------------------------------------------------------------------
// XML 1.0 character and name rules
// ---------------------------------------------------------------------------

fn is_name_start_char(c: char) -> bool {
    c == ':' || c == '_' || c.is_alphabetic()
        || ('\u{00C0}'..='\u{00D6}').contains(&c)
        || ('\u{00D8}'..='\u{00F6}').contains(&c)
        || ('\u{00F8}'..='\u{02FF}').contains(&c)
        || ('\u{0370}'..='\u{037D}').contains(&c)
        || ('\u{037F}'..='\u{1FFF}').contains(&c)
        || ('\u{200C}'..='\u{200D}').contains(&c)
        || ('\u{2070}'..='\u{218F}').contains(&c)
        || ('\u{2C00}'..='\u{2FEF}').contains(&c)
        || ('\u{3001}'..='\u{D7FF}').contains(&c)
        || ('\u{F900}'..='\u{FDCF}').contains(&c)
        || ('\u{FDF0}'..='\u{FFFD}').contains(&c)
        || ('\u{10000}'..='\u{EFFFF}').contains(&c)
}

fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || c.is_ascii_digit()
        || c.is_numeric()
        || c == '-'
        || c == '.'
        || c == '\u{00B7}'
        || ('\u{0300}'..='\u{036F}').contains(&c)
        || ('\u{203F}'..='\u{2040}').contains(&c)
}

pub fn is_xml_name(raw: &[u8]) -> bool {
    if raw.is_empty() {
        return false;
    }
    let s = match std::str::from_utf8(raw) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let mut it = s.chars();
    let first = it.next().unwrap();
    if !is_name_start_char(first) {
        return false;
    }
    it.all(is_name_char)
}

pub fn is_xml_char(code: i64) -> bool {
    code == 0x9
        || code == 0xA
        || code == 0xD
        || (0x20..=0xD7FF).contains(&code)
        || (0xE000..=0xFFFD).contains(&code)
        || (0x10000..=0x10FFFF).contains(&code)
}

fn char_ref_code(digits: &str, base: u32) -> i64 {
    if digits.is_empty() {
        return -1;
    }
    let mut code: i64 = 0;
    for ch in digits.chars() {
        let d = match ch.to_digit(base) {
            Some(d) => d,
            None => return -1,
        };
        code = code * base as i64 + d as i64;
        if code > 0x10FFFF {
            return -1;
        }
    }
    code
}

fn has_forbidden_char(s: &str) -> bool {
    s.chars().any(|c| !is_xml_char(c as i64))
}

/// An attribute value is utf-8, holds no "<", carries only characters XML
/// allows, and every "&" opens a reference that resolves to one.
pub fn attvalue_is_well_formed(value: &[u8]) -> bool {
    if value.contains(&b'<') {
        return false;
    }
    let text = match std::str::from_utf8(value) {
        Ok(t) => t,
        Err(_) => return false,
    };
    if has_forbidden_char(text) {
        return false;
    }
    let bytes = text.as_bytes();
    let mut pos = 0usize;
    while let Some(rel) = memchr::memchr(b'&', &bytes[pos..]) {
        let amp = pos + rel;
        let semi = match memchr::memchr(b';', &bytes[amp + 1..]) {
            Some(r) => amp + 1 + r,
            None => return false,
        };
        let body = &text[amp + 1..semi];
        if body.starts_with("#x") {
            if !is_xml_char(char_ref_code(&body[2..], 16)) {
                return false;
            }
        } else if body.starts_with('#') {
            if !is_xml_char(char_ref_code(&body[1..], 10)) {
                return false;
            }
        } else if !is_xml_name(body.as_bytes()) {
            return false;
        }
        pos = semi + 1;
    }
    true
}

fn name_end(tag: &[u8], mut pos: usize, end: usize) -> i64 {
    let start = pos;
    while pos < end && !NAME_STOP.contains(&tag[pos]) {
        pos += 1;
    }
    if pos == start || !is_xml_name(&tag[start..pos]) {
        return -1;
    }
    pos as i64
}

/// A start tag as XML defines it. `tag` includes both `<` and `>`.
pub fn start_tag_is_well_formed(tag: &[u8]) -> bool {
    if tag.len() < 2 || tag[0] != b'<' || tag[tag.len() - 1] != b'>' {
        return false;
    }
    let mut end = tag.len() - 1; // the closing ">"
    if end > 0 && tag[end - 1] == b'/' {
        end -= 1; // empty-element tag: "/>" carries no space between
    }
    let mut pos = name_end(tag, 1, end);
    if pos == -1 {
        return false;
    }
    let mut seen: Vec<&[u8]> = Vec::new();
    let p = &mut pos;
    let mut idx = *p as usize;
    while idx < end {
        let mut spaced = false;
        while idx < end && XML_WHITESPACE.contains(&tag[idx]) {
            idx += 1;
            spaced = true;
        }
        if idx >= end {
            return true;
        }
        if !spaced {
            return false;
        }
        let stop = name_end(tag, idx, end);
        if stop == -1 {
            return false;
        }
        let name = &tag[idx..stop as usize];
        if seen.contains(&name) {
            return false;
        }
        seen.push(name);
        idx = stop as usize;
        while idx < end && XML_WHITESPACE.contains(&tag[idx]) {
            idx += 1;
        }
        if idx >= end || tag[idx] != b'=' {
            return false;
        }
        idx += 1;
        while idx < end && XML_WHITESPACE.contains(&tag[idx]) {
            idx += 1;
        }
        if idx >= end {
            return false;
        }
        let quote = tag[idx];
        if quote != b'"' && quote != b'\'' {
            return false;
        }
        let close = match memchr::memchr(quote, &tag[idx + 1..end]) {
            Some(r) => idx + 1 + r,
            None => return false,
        };
        if !attvalue_is_well_formed(&tag[idx + 1..close]) {
            return false;
        }
        idx = close + 1;
    }
    true
}

/// An end tag carries a name, optional whitespace, and nothing else.
pub fn end_tag_is_well_formed(tag: &[u8]) -> bool {
    if tag.len() < 3 || tag[0] != b'<' || tag[1] != b'/' || tag[tag.len() - 1] != b'>'
    {
        return false;
    }
    let end = tag.len() - 1;
    let pos = name_end(tag, 2, end);
    if pos == -1 {
        return false;
    }
    let mut idx = pos as usize;
    while idx < end && XML_WHITESPACE.contains(&tag[idx]) {
        idx += 1;
    }
    idx == end
}

// ---------------------------------------------------------------------------
// Per-item document parser (ItemHandler port)
// ---------------------------------------------------------------------------

struct Frame {
    tag: String,
    key: String,
    text: String,
    has_children: bool,
}

#[derive(Default)]
struct Sink {
    tags: Vec<(String, String)>,
    index: std::collections::HashMap<String, usize>,
    appearances: std::collections::HashMap<String, usize>,
    collisions: Vec<(String, String, String)>,
}

impl Sink {
    fn set(&mut self, key: String, value: String) {
        // rstrip("\n") mirrors the Python post-pass.
        let value = value.trim_end_matches('\n').to_string();
        if let Some(&i) = self.index.get(&key) {
            self.tags[i].1 = value;
        } else {
            self.index.insert(key.clone(), self.tags.len());
            self.tags.push((key, value));
        }
    }

    fn number(&mut self, path: &str, tag: &str) -> String {
        let key = match self.appearances.get_mut(path) {
            Some(count) => {
                *count += 1;
                format!("{path}_{count}")
            }
            None => {
                self.appearances.insert(path.to_string(), 0);
                path.to_string()
            }
        };
        let mut key = key;
        if self.index.contains_key(&key) {
            let taken = key.clone();
            loop {
                let count = self.appearances.get_mut(path).unwrap();
                *count += 1;
                key = format!("{path}_{count}");
                if !self.index.contains_key(&key) {
                    break;
                }
            }
            self.collisions
                .push((taken, tag.to_string(), key.clone()));
        }
        key
    }
}

pub struct DocResult {
    pub parsed: Option<Vec<(String, String)>>,
    pub collisions: Vec<(String, String, String)>,
}

fn err() -> DocResult {
    DocResult {
        parsed: None,
        collisions: Vec::new(),
    }
}

/// Parse a whole item document (wrapper included) into flat fields.
pub fn parse_document(doc: &[u8]) -> DocResult {
    let n = doc.len();
    let mut i = 0usize;
    let mut stack: Vec<Frame> = Vec::new();
    let mut sink = Sink::default();
    let mut root_closed = false;
    let mut root_seen = false;

    // Document prolog/epilog: only whitespace, comments, PIs, and a leading
    // DOCTYPE are allowed outside the root element.
    let mut in_prolog = true;

    while i < n {
        if doc[i] != b'<' {
            // Ordinary text.
            if stack.is_empty() {
                // Outside the root: whitespace only.
                while i < n && doc[i] != b'<' {
                    if !XML_WHITESPACE.contains(&doc[i]) {
                        return err();
                    }
                    i += 1;
                }
                continue;
            }
            let (text, next) = match decode_text(doc, i) {
                Some(v) => v,
                None => return err(),
            };
            stack.last_mut().unwrap().text.push_str(&text);
            i = next;
            continue;
        }

        if doc[i..].starts_with(b"<!--") {
            match find_seq(doc, i + 4, b"-->") {
                Some(end) => {
                    i = end + 3;
                    continue;
                }
                None => return err(),
            }
        }
        if doc[i..].starts_with(b"<![CDATA[") {
            if stack.is_empty() {
                return err();
            }
            match find_seq(doc, i + 9, b"]]>") {
                Some(end) => {
                    let raw = &doc[i + 9..end];
                    let text = match std::str::from_utf8(raw) {
                        Ok(t) => t,
                        Err(_) => return err(),
                    };
                    if has_forbidden_char(text) {
                        return err();
                    }
                    stack.last_mut().unwrap().text.push_str(text);
                    i = end + 3;
                    continue;
                }
                None => return err(),
            }
        }
        if doc[i..].starts_with(b"<?") {
            match find_seq(doc, i + 2, b"?>") {
                Some(end) => {
                    i = end + 2;
                    continue;
                }
                None => return err(),
            }
        }
        if doc[i..].starts_with(b"<!DOCTYPE") {
            if !stack.is_empty() || root_seen {
                return err();
            }
            match scan_doctype(doc, i) {
                Some(end) => {
                    in_prolog = true;
                    i = end;
                    continue;
                }
                None => return err(),
            }
        }
        if doc[i..].starts_with(b"<!") {
            return err();
        }

        if i + 1 < n && doc[i + 1] == b'/' {
            // End tag.
            if stack.is_empty() {
                return err();
            }
            let end = match find_tag_end(doc, i + 1) {
                Some(e) => e,
                None => return err(),
            };
            let tag = &doc[i..end];
            if !end_tag_is_well_formed(tag) {
                return err();
            }
            let name = end_tag_name(tag);
            if name != stack.last().unwrap().tag.as_bytes() {
                return err();
            }
            let frame = stack.pop().unwrap();
            let is_root = stack.is_empty();
            emit(&mut sink, frame, is_root);
            i = end;
            if is_root {
                root_closed = true;
                in_prolog = false;
            }
            continue;
        }

        // Start tag.
        if root_closed {
            return err();
        }
        let end = match find_tag_end(doc, i + 1) {
            Some(e) => e,
            None => return err(),
        };
        let tag_bytes = &doc[i..end];
        if !start_tag_is_well_formed(tag_bytes) {
            return err();
        }
        let name = match start_tag_name(tag_bytes) {
            Some(n) => match std::str::from_utf8(n) {
                Ok(s) => s.to_string(),
                Err(_) => return err(),
            },
            None => return err(),
        };
        let self_closing = is_self_closing(tag_bytes);

        if stack.is_empty() {
            root_seen = true;
            in_prolog = false;
            stack.push(Frame {
                tag: name.clone(),
                key: name,
                text: String::new(),
                has_children: false,
            });
        } else {
            stack.last_mut().unwrap().has_children = true;
            let path = if stack.len() > 1 {
                format!("{}/{}", stack.last().unwrap().key, name)
            } else {
                name.clone()
            };
            let key = sink.number(&path, &name);
            stack.push(Frame {
                tag: name,
                key,
                text: String::new(),
                has_children: false,
            });
        }
        i = end;

        if self_closing {
            let frame = stack.pop().unwrap();
            let is_root = stack.is_empty();
            emit(&mut sink, frame, is_root);
            if is_root {
                root_closed = true;
                in_prolog = false;
            }
        }
        let _ = in_prolog;
    }

    if !stack.is_empty() || !root_seen || !root_closed {
        return err();
    }

    DocResult {
        parsed: Some(sink.tags),
        collisions: sink.collisions,
    }
}

fn emit(sink: &mut Sink, frame: Frame, is_root: bool) {
    if is_root {
        if !frame.has_children {
            sink.set(frame.key, frame.text);
        }
        return;
    }
    if !frame.has_children || !frame.text.trim().is_empty() {
        sink.set(frame.key, frame.text);
    }
}

fn find_seq(bytes: &[u8], from: usize, seq: &[u8]) -> Option<usize> {
    memchr::memmem::find(&bytes[from..], seq).map(|r| from + r)
}

/// Position just past the `>` that closes tag starting at `from` (which
/// points at the byte after `<`). A `>` inside quotes closes nothing.
fn find_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    let n = bytes.len();
    while i < n {
        match bytes[i] {
            b'>' => return Some(i + 1),
            b'"' | b'\'' => {
                let quote = bytes[i];
                let close = memchr::memchr(quote, &bytes[i + 1..])?;
                i = i + 1 + close + 1;
            }
            _ => i += 1,
        }
    }
    None
}

fn start_tag_name(tag: &[u8]) -> Option<&[u8]> {
    let end = tag.len() - 1;
    let pos = name_end(tag, 1, end);
    if pos == -1 {
        return None;
    }
    Some(&tag[1..pos as usize])
}

fn end_tag_name(tag: &[u8]) -> &[u8] {
    let end = tag.len() - 1;
    let pos = name_end(tag, 2, end);
    &tag[2..pos as usize]
}

fn is_self_closing(tag: &[u8]) -> bool {
    let mut end = tag.len() - 1; // '>'
    while end > 0 && XML_WHITESPACE.contains(&tag[end - 1]) {
        end -= 1;
    }
    end > 0 && tag[end - 1] == b'/'
}

/// Scan a DOCTYPE declaration, returning the position after its closing `>`.
/// Any `<!ENTITY` inside the internal subset is rejected.
fn scan_doctype(bytes: &[u8], start: usize) -> Option<usize> {
    let n = bytes.len();
    let mut i = start + 9; // past "<!DOCTYPE"
    let mut depth: i32 = 0;
    let mut quote: u8 = 0;
    while i < n {
        if quote != 0 {
            if bytes[i] == quote {
                quote = 0;
            }
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(b"<!--") {
            let end = find_seq(bytes, i + 4, b"-->")?;
            i = end + 3;
            continue;
        }
        if bytes[i..].starts_with(b"<?") {
            let end = find_seq(bytes, i + 2, b"?>")?;
            i = end + 2;
            continue;
        }
        if bytes[i..].starts_with(b"<!ENTITY") {
            return None;
        }
        match bytes[i] {
            b'"' | b'\'' => {
                quote = bytes[i];
                i += 1;
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                i += 1;
            }
            b'>' => {
                if depth <= 0 {
                    return Some(i + 1);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Decode a run of character data up to the next `<`.
fn decode_text(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let n = bytes.len();
    let mut i = start;
    let mut out = String::new();
    while i < n && bytes[i] != b'<' {
        if bytes[i] == b'&' {
            let (decoded, next) = decode_ref(bytes, i)?;
            out.push_str(&decoded);
            i = next;
            continue;
        }
        let run_start = i;
        while i < n && bytes[i] != b'<' && bytes[i] != b'&' {
            i += 1;
        }
        let run = std::str::from_utf8(&bytes[run_start..i]).ok()?;
        if has_forbidden_char(run) {
            return None;
        }
        out.push_str(run);
    }
    Some((out, i))
}

fn decode_ref(bytes: &[u8], amp: usize) -> Option<(String, usize)> {
    let semi = memchr::memchr(b';', &bytes[amp + 1..])? + amp + 1;
    let body = &bytes[amp + 1..semi];
    let replacement: String = match body {
        b"amp" => "&".to_string(),
        b"lt" => "<".to_string(),
        b"gt" => ">".to_string(),
        b"quot" => "\"".to_string(),
        b"apos" => "'".to_string(),
        _ => {
            let text = std::str::from_utf8(body).ok()?;
            if let Some(hex) = text.strip_prefix("#x") {
                let code = char_ref_code(hex, 16);
                if !is_xml_char(code) {
                    return None;
                }
                char::from_u32(code as u32)?.to_string()
            } else if let Some(dec) = text.strip_prefix('#') {
                let code = char_ref_code(dec, 10);
                if !is_xml_char(code) {
                    return None;
                }
                char::from_u32(code as u32)?.to_string()
            } else {
                return None; // undefined entity
            }
        }
    };
    Some((replacement, semi + 1))
}

// ---------------------------------------------------------------------------
// Borrow-first item fields (adapter path)
// ---------------------------------------------------------------------------
//
// The streaming path flattens a whole document into owned `String`s. The
// rypipe `RecordParser` can do better: leaf text is usually a single
// entity-free run, so it can be borrowed straight from the input and handed
// to the engine as `Cow::Borrowed`. Only text that spans multiple runs
// (mixed content) or contains entities is materialized.

/// A flattened field value, borrowing the input when it can.
pub enum FlatVal<'a> {
    Empty,
    Borrowed(&'a str),
    Owned(String),
}

impl<'a> FlatVal<'a> {
    /// Python's `str.rstrip("\n")` on the flattened value.
    pub fn rstrip_newlines(self) -> std::borrow::Cow<'a, str> {
        match self {
            FlatVal::Empty => std::borrow::Cow::Borrowed(""),
            FlatVal::Borrowed(s) => std::borrow::Cow::Borrowed(s.trim_end_matches('\n')),
            FlatVal::Owned(mut s) => {
                while s.ends_with('\n') {
                    s.pop();
                }
                std::borrow::Cow::Owned(s)
            }
        }
    }

    pub fn trim_is_empty(&self) -> bool {
        match self {
            FlatVal::Empty => true,
            FlatVal::Borrowed(s) => s.trim().is_empty(),
            FlatVal::Owned(s) => s.trim().is_empty(),
        }
    }
}

/// Accumulates an element's direct character data, borrowing a single run.
#[derive(Default)]
struct TextBuf {
    has: bool,
    start: usize,
    end: usize,
    owned: Option<String>,
}

impl TextBuf {
    fn materialize(&mut self, content: &[u8]) {
        if self.owned.is_none() {
            let mut out = String::new();
            if self.has {
                out.push_str(std::str::from_utf8(&content[self.start..self.end]).unwrap_or(""));
            }
            self.owned = Some(out);
            self.has = false;
        }
    }

    fn push_run(&mut self, content: &[u8], start: usize, end: usize) {
        let run = std::str::from_utf8(&content[start..end]).unwrap_or("");
        if let Some(owned) = self.owned.as_mut() {
            owned.push_str(run);
            return;
        }
        if !self.has {
            self.has = true;
            self.start = start;
            self.end = end;
        } else if self.end == start {
            self.end = end;
        } else {
            self.materialize(content);
            self.owned.as_mut().unwrap().push_str(run);
        }
    }

    fn push_owned(&mut self, content: &[u8], text: String) {
        self.materialize(content);
        self.owned.as_mut().unwrap().push_str(&text);
    }

    fn is_empty(&self) -> bool {
        if let Some(owned) = &self.owned {
            return owned.is_empty();
        }
        !self.has || self.start == self.end
    }

    fn into_val(self, content: &[u8]) -> FlatVal<'_> {
        if let Some(owned) = self.owned {
            FlatVal::Owned(owned)
        } else if self.has {
            FlatVal::Borrowed(std::str::from_utf8(&content[self.start..self.end]).unwrap_or(""))
        } else {
            FlatVal::Empty
        }
    }
}

struct TextFrame {
    tag: String,
    key: String,
    text: TextBuf,
    has_children: bool,
}

fn read_text_run(content: &[u8], mut i: usize, target: &mut TextBuf) -> Option<usize> {
    let n = content.len();
    while i < n && content[i] != b'<' {
        if content[i] == b'&' {
            let (decoded, next) = decode_ref(content, i)?;
            target.push_owned(content, decoded);
            i = next;
            continue;
        }
        let start = i;
        while i < n && content[i] != b'<' && content[i] != b'&' {
            i += 1;
        }
        let run = std::str::from_utf8(&content[start..i]).ok()?;
        if has_forbidden_char(run) {
            return None;
        }
        target.push_run(content, start, i);
    }
    Some(i)
}

fn number_key(
    path: &str,
    appearances: &mut std::collections::HashMap<String, usize>,
    keys: &mut std::collections::HashSet<String>,
) -> String {
    let mut key = match appearances.get_mut(path) {
        Some(count) => {
            *count += 1;
            format!("{path}_{count}")
        }
        None => {
            appearances.insert(path.to_string(), 0);
            path.to_string()
        }
    };
    // `keys` holds emitted keys only, matching the streaming parser's
    // collision rule (it checks the values already in `tags`).
    while keys.contains(&key) {
        let count = appearances.get_mut(path).unwrap();
        *count += 1;
        key = format!("{path}_{count}");
    }
    key
}

/// Flatten an item's inner bytes into ordered `(key, value)` fields.
///
/// Returns `None` when the content is not well formed (the item is dropped).
/// Leaf values borrow `content` where possible.
pub fn parse_item_flat<'a>(content: &'a [u8], sep: &str) -> Option<Vec<(String, FlatVal<'a>)>> {
    let n = content.len();
    let mut i = 0usize;
    let mut stack: Vec<TextFrame> = Vec::new();
    let mut fields: Vec<(String, FlatVal<'_>)> = Vec::new();
    let mut appearances: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut root = TextBuf::default();
    let mut seen_child = false;

    while i < n {
        if content[i] != b'<' {
            let target = match stack.last_mut() {
                Some(frame) => &mut frame.text,
                None => &mut root,
            };
            i = read_text_run(content, i, target)?;
            continue;
        }

        if content[i..].starts_with(b"<!--") {
            i = find_seq(content, i + 4, b"-->")? + 3;
            continue;
        }
        if content[i..].starts_with(b"<![CDATA[") {
            let end = find_seq(content, i + 9, b"]]>")?;
            let raw = std::str::from_utf8(&content[i + 9..end]).ok()?;
            if has_forbidden_char(raw) {
                return None;
            }
            let target = match stack.last_mut() {
                Some(frame) => &mut frame.text,
                None => &mut root,
            };
            target.push_run(content, i + 9, end);
            i = end + 3;
            continue;
        }
        if content[i..].starts_with(b"<?") {
            i = find_seq(content, i + 2, b"?>")? + 2;
            continue;
        }
        if content[i..].starts_with(b"<!") {
            return None;
        }

        if i + 1 < n && content[i + 1] == b'/' {
            if stack.is_empty() {
                return None;
            }
            let end = find_tag_end(content, i + 1)?;
            let tag = &content[i..end];
            if !end_tag_is_well_formed(tag) {
                return None;
            }
            let name = end_tag_name(tag);
            let frame = stack.pop().unwrap();
            if name != frame.tag.as_bytes() {
                return None;
            }
            i = end;
            let text = frame.text.into_val(content);
            if !frame.has_children || !text.trim_is_empty() {
                keys.insert(frame.key.clone());
                fields.push((frame.key, text));
            }
            continue;
        }

        let end = find_tag_end(content, i + 1)?;
        let tag = &content[i..end];
        if !start_tag_is_well_formed(tag) {
            return None;
        }
        let name = match start_tag_name(tag) {
            Some(raw) => std::str::from_utf8(raw).ok()?.to_string(),
            None => return None,
        };
        let self_closing = is_self_closing(tag);
        let frame = if stack.is_empty() {
            seen_child = true;
            TextFrame {
                tag: name.clone(),
                key: number_key(&name, &mut appearances, &mut keys),
                text: TextBuf::default(),
                has_children: false,
            }
        } else {
            stack.last_mut().unwrap().has_children = true;
            // The implicit wrapper is not on the stack: any ancestor here
            // prefixes the path.
            let path = format!("{}/{}", stack.last().unwrap().key, name);
            let key = number_key(&path, &mut appearances, &mut keys);
            TextFrame {
                tag: name,
                key,
                text: TextBuf::default(),
                has_children: false,
            }
        };
        i = end;
        stack.push(frame);
        if self_closing {
            let frame = stack.pop().unwrap();
            let text = frame.text.into_val(content);
            if !frame.has_children || !text.trim_is_empty() {
                keys.insert(frame.key.clone());
                fields.push((frame.key, text));
            }
        }
    }

    if !stack.is_empty() {
        return None;
    }
    if !seen_child {
        // The implicit wrapper is a leaf (bare text or empty).
        return Some(vec![(sep.to_string(), root.into_val(content))]);
    }
    Some(fields)
}
