use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList};

use rypipe_core::{MemoryBudget, Pipeline};
use rypipe_python::{
    execution_plan_from_kwargs, py_err_from_rypipe, record_batches_to_pyarrow_batches,
    record_batches_to_pyarrow_table,
};

mod parser;
mod scan;
mod splitter;
mod tokenizer;
mod xml;

use parser::XmlParser;
use splitter::XmlSplitter;
use tokenizer::{Log, Next};

/// Parse a memory string ("64MiB", "1GB") or an int into bytes.
fn memory_bytes(value: &Bound<'_, PyAny>) -> PyResult<usize> {
    if let Ok(bytes) = value.extract::<usize>() {
        return Ok(bytes.max(1));
    }
    let text: String = value.extract()?;
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(text.len());
    let (num, unit) = text.split_at(split);
    let num: f64 = num
        .parse()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("invalid memory value"))?;
    let mult: f64 = match unit.trim().to_ascii_uppercase().as_str() {
        "" | "B" => 1.0,
        "KB" => 1_000.0,
        "MB" => 1_000_000.0,
        "GB" => 1_000_000_000.0,
        "KIB" => 1024.0,
        "MIB" => 1024.0 * 1024.0,
        "GIB" => 1024.0 * 1024.0 * 1024.0,
        other => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown memory unit: {other:?}"
            )))
        }
    };
    Ok(((num * mult) as usize).max(1))
}

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

#[pyfunction]
#[pyo3(signature = (path, separator_tag="item".to_string(), field_mapping=None, drop_fields=None,
    filter=None, field_types=None, dictionary_columns=None, schema=None, auto_dict=false,
    auto_dict_threshold=None, auto_dict_max_size=None, strict_types=false,
    max_split_chunks=None, observer=None, use_mmap=false, prefault=false))]
#[allow(clippy::too_many_arguments)]
fn read_xml(
    py: Python<'_>,
    path: String,
    separator_tag: String,
    field_mapping: Option<HashMap<String, String>>,
    drop_fields: Option<Vec<String>>,
    filter: Option<Bound<'_, PyAny>>,
    field_types: Option<HashMap<String, String>>,
    dictionary_columns: Option<Vec<String>>,
    schema: Option<Vec<String>>,
    auto_dict: bool,
    auto_dict_threshold: Option<f64>,
    auto_dict_max_size: Option<usize>,
    strict_types: bool,
    max_split_chunks: Option<usize>,
    observer: Option<Bound<'_, PyAny>>,
    use_mmap: bool,
    prefault: bool,
) -> PyResult<Py<PyAny>> {
    let plan = execution_plan_from_kwargs(
        field_mapping,
        drop_fields,
        filter.as_ref(),
        field_types,
        dictionary_columns,
        schema,
        auto_dict,
        auto_dict_threshold,
        auto_dict_max_size,
        strict_types,
        max_split_chunks,
        observer.as_ref(),
    )?;
    let batch = py
        .detach(|| {
            Pipeline::new(
                XmlSplitter::new(&separator_tag),
                XmlParser::new(&separator_tag),
            )
            .with_plan(plan)
            .read_path(&path, use_mmap, prefault)
        })
        .map_err(py_err_from_rypipe)?;
    record_batches_to_pyarrow_table(py, &[batch]).map(|table| table.unbind())
}

#[pyfunction]
#[pyo3(signature = (path, separator_tag="item".to_string(), chunks=4, field_mapping=None,
    drop_fields=None, filter=None, field_types=None, dictionary_columns=None, schema=None,
    auto_dict=false, auto_dict_threshold=None, auto_dict_max_size=None, strict_types=false,
    max_split_chunks=None, observer=None, use_mmap=true, prefault=false))]
#[allow(clippy::too_many_arguments)]
fn read_xml_par(
    py: Python<'_>,
    path: String,
    separator_tag: String,
    chunks: usize,
    field_mapping: Option<HashMap<String, String>>,
    drop_fields: Option<Vec<String>>,
    filter: Option<Bound<'_, PyAny>>,
    field_types: Option<HashMap<String, String>>,
    dictionary_columns: Option<Vec<String>>,
    schema: Option<Vec<String>>,
    auto_dict: bool,
    auto_dict_threshold: Option<f64>,
    auto_dict_max_size: Option<usize>,
    strict_types: bool,
    max_split_chunks: Option<usize>,
    observer: Option<Bound<'_, PyAny>>,
    use_mmap: bool,
    prefault: bool,
) -> PyResult<Py<PyAny>> {
    let plan = execution_plan_from_kwargs(
        field_mapping,
        drop_fields,
        filter.as_ref(),
        field_types,
        dictionary_columns,
        schema,
        auto_dict,
        auto_dict_threshold,
        auto_dict_max_size,
        strict_types,
        max_split_chunks,
        observer.as_ref(),
    )?;
    let batches = py
        .detach(|| {
            Pipeline::new(
                XmlSplitter::new(&separator_tag),
                XmlParser::new(&separator_tag),
            )
            .with_plan(plan)
            .read_path_par(&path, chunks, use_mmap, prefault)
        })
        .map_err(py_err_from_rypipe)?;
    record_batches_to_pyarrow_table(py, &batches).map(|table| table.unbind())
}

/// Bounded-memory read: returns a list of `pyarrow.RecordBatch`.
#[pyfunction]
#[pyo3(signature = (path, separator_tag="item".to_string(), memory=None, field_mapping=None,
    drop_fields=None, filter=None, field_types=None, dictionary_columns=None, schema=None,
    auto_dict=false, auto_dict_threshold=None, auto_dict_max_size=None, strict_types=false,
    max_split_chunks=None, observer=None, use_mmap=true, prefault=false))]
#[allow(clippy::too_many_arguments)]
fn read_xml_stream(
    py: Python<'_>,
    path: String,
    separator_tag: String,
    memory: Option<Bound<'_, PyAny>>,
    field_mapping: Option<HashMap<String, String>>,
    drop_fields: Option<Vec<String>>,
    filter: Option<Bound<'_, PyAny>>,
    field_types: Option<HashMap<String, String>>,
    dictionary_columns: Option<Vec<String>>,
    schema: Option<Vec<String>>,
    auto_dict: bool,
    auto_dict_threshold: Option<f64>,
    auto_dict_max_size: Option<usize>,
    strict_types: bool,
    max_split_chunks: Option<usize>,
    observer: Option<Bound<'_, PyAny>>,
    use_mmap: bool,
    prefault: bool,
) -> PyResult<Py<PyAny>> {
    let _ = use_mmap; // the bounded path reads through InputBuffer directly
    let budget = match memory {
        Some(value) => MemoryBudget::new(memory_bytes(&value)?),
        None => MemoryBudget::new(64 * 1024 * 1024),
    };
    let plan = execution_plan_from_kwargs(
        field_mapping,
        drop_fields,
        filter.as_ref(),
        field_types,
        dictionary_columns,
        schema,
        auto_dict,
        auto_dict_threshold,
        auto_dict_max_size,
        strict_types,
        max_split_chunks,
        observer.as_ref(),
    )?;
    let batches = py
        .detach(|| {
            Pipeline::new(
                XmlSplitter::new(&separator_tag),
                XmlParser::new(&separator_tag),
            )
            .with_plan(plan)
            .read_path_stream(&path, budget, prefault)
        })
        .map_err(py_err_from_rypipe)?;
    record_batches_to_pyarrow_batches(py, &batches).map(|list| list.unbind().into_any())
}

#[pymodule]
fn _xmlstreamer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_document, m)?)?;
    m.add_function(wrap_pyfunction!(attvalue_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(start_tag_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(end_tag_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(is_xml_name, m)?)?;
    m.add_function(wrap_pyfunction!(read_xml, m)?)?;
    m.add_function(wrap_pyfunction!(read_xml_par, m)?)?;
    m.add_function(wrap_pyfunction!(read_xml_stream, m)?)?;
    m.add_class::<Tokenizer>()?;
    Ok(())
}
