"""Registration of xmlstreamer as a `rypipe` adapter.

Importing this module (which `xmlstreamer/__init__.py` does) makes
``rypipe.read("feed.xml", separator_tag="item")`` work.
"""

from __future__ import annotations

from typing import Any, Iterator, Optional

import pyarrow as pa

from . import _xmlstreamer

__all__ = ["XmlAdapter"]


class XmlAdapter:
    """Plain adapter object registered with rypipe."""

    def read(self, path: str, **kwargs: Any) -> pa.Table:
        return _xmlstreamer.read_xml(str(path), **kwargs)

    def iter_record_batches(
        self,
        path: str,
        memory: Any = "64MiB",
        batch_size: Optional[int] = None,
        **kwargs: Any,
    ) -> Iterator[pa.RecordBatch]:
        yield from _xmlstreamer.read_xml_stream(
            str(path), memory=memory, **kwargs
        )


def _register() -> None:
    try:
        import rypipe
    except Exception:  # pragma: no cover - rypipe is a hard dependency
        return
    rypipe.register_adapter("xmlstreamer", XmlAdapter(), extensions=[".xml"])


_register()
