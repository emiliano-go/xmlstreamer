use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

mod tokenizer;
mod xml;

use tokenizer::{Log, Next};

/// Parse a whole item document into `(flat_dict_or_None, collisions)`.
#[pyfunction]
fn parse_document(
    py: Python<'_>,
    doc: &[u8],
) -> PyResult<(Option<Py<PyDict>>, Vec<(String, String, String)>)> {
    let result = xml::parse_document(doc);
    let parsed = match result.parsed {
        Some(tags) => {
            let dict = PyDict::new(py);
            for (key, value) in tags {
                dict.set_item(key, value)?;
            }
            Some(dict.into())
        }
        None => None,
    };
    Ok((parsed, result.collisions))
}

#[pyfunction]
fn attvalue_is_well_formed(value: &[u8]) -> bool {
    xml::attvalue_is_well_formed(value)
}

#[pyfunction]
fn start_tag_is_well_formed(tag: &[u8]) -> bool {
    xml::start_tag_is_well_formed(tag)
}

#[pyfunction]
fn end_tag_is_well_formed(tag: &[u8]) -> bool {
    xml::end_tag_is_well_formed(tag)
}

#[pyfunction]
fn is_xml_name(raw: &[u8]) -> bool {
    xml::is_xml_name(raw)
}

/// Streaming item tokenizer backed by the rypipe-core scan primitives.
#[pyclass]
struct Tokenizer {
    engine: tokenizer::Engine,
}

#[pymethods]
impl Tokenizer {
    #[new]
    #[pyo3(signature = (separator_tag, buffer_size, max_section_size, on_limit_text, deadline_secs=None))]
    fn new(
        separator_tag: &str,
        buffer_size: usize,
        max_section_size: i64,
        on_limit_text: bool,
        deadline_secs: Option<f64>,
    ) -> Self {
        Tokenizer {
            engine: tokenizer::Engine::new(
                separator_tag,
                buffer_size,
                max_section_size,
                on_limit_text,
                deadline_secs,
            ),
        }
    }

    fn feed(&mut self, chunk: &[u8]) {
        self.engine.feed(chunk);
    }

    fn finish(&mut self) {
        self.engine.finish();
    }

    fn state(&self) -> u8 {
        match self.engine.state() {
            tokenizer::State::SeekOpen => 0,
            tokenizer::State::SeekClose => 1,
            tokenizer::State::ItemParsed => 2,
            tokenizer::State::Eof => 3,
        }
    }

    fn malformed_tags(&self) -> usize {
        self.engine.malformed_tags()
    }

    /// Drain the warnings the scan produced since the last call.
    fn take_logs<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for log in self.engine.take_logs() {
            match log {
                Log::Unterminated {
                    opener,
                    limit,
                    markup,
                } => {
                    list.append((
                        "unterminated",
                        PyBytes::new(py, &opener),
                        limit,
                        markup,
                    ))?;
                }
                Log::MalformedTag { tag } => {
                    list.append(("malformed_tag", PyBytes::new(py, &tag)))?;
                }
                Log::EndedInItem { discarded } => {
                    list.append(("ended_in_item", discarded))?;
                }
                Log::SectionEof { opener, discarded } => {
                    list.append((
                        "section_eof",
                        PyBytes::new(py, &opener),
                        discarded,
                    ))?;
                }
                Log::FeedTimeout => {
                    list.append(("feed_timeout",))?;
                }
            }
        }
        Ok(list)
    }

    /// Return `(code, payload)`: 0 EOF, 1 need-more, 2 item.
    fn next_item<'py>(
        &mut self,
        py: Python<'py>,
    ) -> PyResult<(u8, Py<PyAny>)> {
        match self.engine.next_item() {
            Next::Eof => Ok((0, py.None())),
            Next::NeedMore => Ok((1, py.None())),
            Next::Item(item) => {
                let parsed = match item.parsed {
                    Some(tags) => {
                        let dict = PyDict::new(py);
                        for (key, value) in tags {
                            dict.set_item(key, value)?;
                        }
                        Some(dict.unbind())
                    }
                    None => None,
                };
                let payload = (
                    PyBytes::new(py, &item.content),
                    parsed,
                    item.replaced,
                    item.collisions,
                );
                let payload_obj: Py<PyAny> =
                    payload.into_pyobject(py)?.unbind().into_any();
                Ok((2, payload_obj))
            }
        }
    }
}

#[pymodule]
fn _xmlstreamer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_document, m)?)?;
    m.add_function(wrap_pyfunction!(attvalue_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(start_tag_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(end_tag_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(is_xml_name, m)?)?;
    m.add_class::<Tokenizer>()?;
    Ok(())
}
