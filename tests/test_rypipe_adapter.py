"""The rypipe adapter must agree with the legacy streaming parser.

The adapter is a second engine over the same contract; these tests pin the
`{path: text}` output (flat keys, numbering, sections, empty/malformed items)
against `Tokenizer`, for the columnar, parallel and bounded-streaming paths.
"""

import pyarrow as pa
import pytest
import rypipe

from feedlib import build_feed, chunked

from xmlstreamer import Tokenizer, XmlSource, col
from xmlstreamer import _xmlstreamer

# (name, feed, separator_tag, expected) — expected mirrors legacy behaviour.
CASES = [
    (
        "flat",
        b"<feed><item><t>a</t><n>1</n></item><item><t>b</t><n>2</n></item></feed>",
        "item",
        [{"t": "a", "n": "1"}, {"t": "b", "n": "2"}],
    ),
    (
        "repeated_siblings",
        b"<item><cat>a</cat><cat>b</cat><cat>c</cat></item>",
        "item",
        [{"cat": "a", "cat_1": "b", "cat_2": "c"}],
    ),
    (
        "nested_paths",
        b"<item><title>A</title><location><city>V</city>"
        b"<country>AA</country></location></item>",
        "item",
        [{"title": "A", "location/city": "V", "location/country": "AA"}],
    ),
    (
        "numbered_parent",
        b"<item><group><v>1</v></group><group><v>2</v></group></item>",
        "item",
        [{"group/v": "1", "group_1/v": "2"}],
    ),
    ("mixed_content", b"<item><a>x<b>y</b>z</a></item>", "item", [{"a/b": "y", "a": "xz"}]),
    (
        "cdata",
        b"<item><d><![CDATA[<b>Hi</b> & bye]]></d></item>",
        "item",
        [{"d": "<b>Hi</b> & bye"}],
    ),
    ("entities", b"<item><t>a &amp; b &lt;3</t></item>", "item", [{"t": "a & b <3"}]),
    ("separator_attrs", b'<feed><item id="7"><t>x</t></item></feed>', "item", [{"t": "x"}]),
    (
        "comment_ghost",
        b"<feed><!-- <item><t>fake</t></item> --><item><t>real</t></item></feed>",
        "item",
        [{"t": "real"}],
    ),
    (
        "cdata_ghost",
        b"<feed><![CDATA[<item><t>fake</t></item>]]><item><t>real</t></item></feed>",
        "item",
        [{"t": "real"}],
    ),
    (
        "malformed_item_between",
        b"<feed><item><t>a</t></item><item><t>b</item>"
        b"<item><t>c</t></item></feed>",
        "item",
        [{"t": "a"}, {"t": "c"}],
    ),
    (
        "empty_items",
        b"<feed><item/><item>   </item><item><t>real</t></item></feed>",
        "item",
        [{"t": "real"}],
    ),
    (
        "bare_text_items",
        b"<catalog><isbn>978-0</isbn><isbn>978-1</isbn></catalog>",
        "isbn",
        [{"isbn": "978-0"}, {"isbn": "978-1"}],
    ),
    (
        "key_collision",
        b"<item><x>a</x><x_1>natural</x_1><x>repeat</x></item>",
        "item",
        [{"x": "a", "x_1": "natural", "x_2": "repeat"}],
    ),
]


def legacy_items(feed: bytes, separator_tag: str) -> list:
    tokenizer = Tokenizer(
        feed_generator=chunked(feed, 7),
        separator_tag=separator_tag,
        buffer_size=64,
    )
    out = []
    while (item := tokenizer.get_item()) is not None:
        if item.parsed_content is not None:
            out.append(item.parsed_content)
    return out


def _write(tmp_path, feed: bytes) -> str:
    path = tmp_path / "feed.xml"
    path.write_bytes(feed)
    return str(path)


@pytest.mark.parametrize("name,feed,sep,expected", CASES, ids=[c[0] for c in CASES])
def test_columnar_matches_legacy(tmp_path, name, feed, sep, expected):
    table = _xmlstreamer.read_xml(_write(tmp_path, feed), separator_tag=sep)
    assert table.to_pylist() == expected
    assert table.to_pylist() == legacy_items(feed, sep)


@pytest.mark.parametrize("name,feed,sep,expected", CASES, ids=[c[0] for c in CASES])
def test_parallel_matches_legacy(tmp_path, name, feed, sep, expected):
    table = _xmlstreamer.read_xml_par(
        _write(tmp_path, feed), separator_tag=sep, chunks=4
    )
    assert table.to_pylist() == expected


@pytest.mark.parametrize("name,feed,sep,expected", CASES, ids=[c[0] for c in CASES])
def test_streaming_matches_legacy(tmp_path, name, feed, sep, expected):
    batches = _xmlstreamer.read_xml_stream(
        _write(tmp_path, feed), separator_tag=sep, memory="1MiB"
    )
    rows = [row for batch in batches for row in batch.to_pylist()]
    assert rows == expected


def test_order_is_preserved_over_many_items(tmp_path):
    feed = build_feed([{"n": str(i)} for i in range(5000)])
    expected = [{"n": str(i)} for i in range(5000)]
    path = _write(tmp_path, feed)
    assert _xmlstreamer.read_xml(path, separator_tag="item").to_pylist() == expected
    assert (
        _xmlstreamer.read_xml_par(path, separator_tag="item", chunks=8).to_pylist()
        == expected
    )


def test_rypipe_read_registration(tmp_path):
    feed = build_feed([{"t": "a"}, {"t": "b"}])
    table = rypipe.read(_write(tmp_path, feed), separator_tag="item")
    assert isinstance(table, pa.Table)
    assert table.to_pylist() == [{"t": "a"}, {"t": "b"}]


def test_source_pipeline_and_sinks(tmp_path):
    feed = build_feed([{"t": "a", "n": "1"}, {"t": "b", "n": "2"}, {"t": "c", "n": "3"}])
    source = XmlSource(_write(tmp_path, feed), separator_tag="item")
    result = (source | rypipe.CastTypes({"n": int}) | rypipe.FilterRows(col("n") > 1)).to_arrow()
    assert result.to_pylist() == [{"t": "b", "n": 2}, {"t": "c", "n": 3}]
    assert result.schema.field("n").type == pa.int64()


def test_source_streaming_iter_record_batches(tmp_path):
    feed = build_feed([{"n": str(i)} for i in range(2000)])
    source = XmlSource(_write(tmp_path, feed), separator_tag="item")
    total = sum(batch.num_rows for batch in source.iter_record_batches(memory="64KiB"))
    assert total == 2000


def test_parallel_streaming_preserves_order_and_matches(tmp_path):
    feed = build_feed([{"n": str(i)} for i in range(5000)])
    path = _write(tmp_path, feed)
    expected = [{"n": str(i)} for i in range(5000)]

    sequential = [
        row
        for batch in _xmlstreamer.read_xml_stream(
            path, separator_tag="item", memory="64KiB"
        )
        for row in batch.to_pylist()
    ]
    parallel = [
        row
        for batch in _xmlstreamer.iter_xml_batches_par(
            path, separator_tag="item", threads=4, memory="1MiB"
        )
        for row in batch.to_pylist()
    ]
    assert sequential == expected
    assert parallel == expected


def test_source_parallel_streaming_via_iter_record_batches(tmp_path):
    feed = build_feed([{"n": str(i)} for i in range(2000)])
    source = XmlSource(_write(tmp_path, feed), separator_tag="item")
    rows = [
        row
        for batch in source.iter_record_batches(memory="1MiB", threads=4)
        for row in batch.to_pylist()
    ]
    assert rows == [{"n": str(i)} for i in range(2000)]


def test_projection_drop_fields(tmp_path):
    feed = build_feed([{"t": "a", "n": "1"}, {"t": "b", "n": "2"}])
    table = _xmlstreamer.read_xml(
        _write(tmp_path, feed), separator_tag="item", drop_fields=["n"]
    )
    assert table.column_names == ["t"]
