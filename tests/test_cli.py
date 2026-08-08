"""The command line is the packaged recipe 8: feed in, JSONL out."""

import json

import pytest

from feedlib import build_feed

from xmlstreamer.__main__ import main


def out_lines(capsys):
    captured = capsys.readouterr()
    return [json.loads(line) for line in captured.out.splitlines()], captured.err


def test_streams_feed_as_jsonl(http_server, capsys):
    url = http_server.serve(
        "/cli.xml", build_feed([{"t": "one"}, {"t": "two"}, {"t": "three"}])
    )
    assert main([url]) == 0
    items, err = out_lines(capsys)
    assert items == [{"t": "one"}, {"t": "two"}, {"t": "three"}]
    assert err == ""


def test_custom_separator_tag(http_server, capsys):
    body = b"<catalog><book><t>a</t></book><book><t>b</t></book></catalog>"
    url = http_server.serve("/books.xml", body)
    assert main([url, "--tag", "book"]) == 0
    items, _ = out_lines(capsys)
    assert [i["t"] for i in items] == ["a", "b"]


@pytest.mark.parametrize("extra", [[], ["--limit", "2"]])
def test_downloads_the_feed_once(http_server, capsys, extra):
    # Iterating a StreamInterpreter restarts the run: handing the loop
    # an already started iterator downloads the whole feed twice.
    url = http_server.serve(
        "/once.xml", build_feed([{"t": "a"}, {"t": "b"}, {"t": "c"}])
    )
    assert main([url] + extra) == 0
    capsys.readouterr()
    assert len(http_server.requests) == 1


def test_construction_errors_stay_one_liners(capsys):
    # Argument validation happens in the constructor: it must land in
    # the CLI's error contract, not escape as a traceback.
    assert main(["http://unused.invalid/feed", "--tag", "   "]) == 1
    captured = capsys.readouterr()
    assert "xmlstreamer:" in captured.err
    assert "Traceback" not in captured.err


def test_limit_consumes_exactly_n_items(monkeypatch, capsys):
    # "Sample N" must pull N items from the source, not N+1: the extra
    # pull costs a parse and, in stream mode, a network wait.
    pulls = {"n": 0}

    class FakeInterpreter:
        def __init__(self, **kwargs):
            pass

        def __iter__(self):
            return self

        def __next__(self):
            pulls["n"] += 1
            return {"n": str(pulls["n"])}

        def __enter__(self):
            return self

        def __exit__(self, *args):
            pass

    monkeypatch.setattr(
        "xmlstreamer.__main__.StreamInterpreter", FakeInterpreter
    )
    assert main(["http://unused.invalid/feed", "--limit", "2"]) == 0
    items, _ = out_lines(capsys)
    assert [i["n"] for i in items] == ["1", "2"]
    assert pulls["n"] == 2


def test_limit_rejects_negative_values(capsys):
    with pytest.raises(SystemExit) as excinfo:
        main(["http://unused.invalid/feed", "--limit", "-1"])
    assert excinfo.value.code == 2
    assert "--limit" in capsys.readouterr().err


def test_limit_stops_early_and_zero_emits_nothing(http_server, capsys):
    feed = build_feed([{"n": str(i)} for i in range(50)])
    url = http_server.serve("/lim.xml", feed)
    assert main([url, "--limit", "2"]) == 0
    items, _ = out_lines(capsys)
    assert [i["n"] for i in items] == ["0", "1"]

    url2 = http_server.serve("/lim0.xml", feed)
    assert main([url2, "--limit", "0"]) == 0
    items, _ = out_lines(capsys)
    assert items == []


def test_nested_output(http_server, capsys):
    body = (
        b"<feed><item><publisher><city>Alba</city></publisher></item></feed>"
    )
    url = http_server.serve("/nested.xml", body)
    assert main([url, "--nested"]) == 0
    items, _ = out_lines(capsys)
    assert items == [{"publisher": {"city": "Alba"}}]


def test_stream_mode_flag_works(http_server, capsys):
    url = http_server.serve("/sm.xml", build_feed([{"t": "x"}]))
    assert main([url, "--mode", "stream"]) == 0
    items, _ = out_lines(capsys)
    assert items == [{"t": "x"}]


def test_errors_exit_nonzero_with_clean_message(capsys):
    rc = main(["gopher://example.invalid/feed"])
    assert rc == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "xmlstreamer:" in captured.err
    assert "Traceback" not in captured.err


def test_version_flag(capsys):
    import xmlstreamer

    with pytest.raises(SystemExit) as excinfo:
        main(["--version"])
    assert excinfo.value.code == 0
    assert xmlstreamer.__version__ in capsys.readouterr().out
