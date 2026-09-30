use pyo3::prelude::*;
use pyo3::types::PyDict;

mod xml;

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

#[pymodule]
fn _xmlstreamer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_document, m)?)?;
    m.add_function(wrap_pyfunction!(attvalue_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(start_tag_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(end_tag_is_well_formed, m)?)?;
    m.add_function(wrap_pyfunction!(is_xml_name, m)?)?;
    Ok(())
}
