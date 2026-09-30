"""Pipeline-capable `rypipe` source for xmlstreamer feeds.

Follows the crxml formula: subclass `rypipe.Adapter`, implement `read`, and
users get the `|` pipeline, caching, and every sink for free. The parse runs
entirely in the Rust rypipe adapter (`xmlstreamer._xmlstreamer`).
"""

from __future__ import annotations

from typing import Any, Iterator, Optional

import pyarrow as pa

from rypipe import Adapter

from . import _xmlstreamer

__all__ = ["XmlSource"]


class XmlSource(Adapter):
    """A row source over an XML feed, one item per record.

    ``separator_tag`` names the record element; everything else follows the
    rypipe `Adapter` contract (``schema``, ``field_types``, ``drop_fields``,
    ``filter``, ``use_mmap``, ...).
    """

    __slots__ = ("_separator_tag",)

    def __init__(
        self,
        path,
        *,
        separator_tag: str = "item",
        **kwargs: Any,
    ) -> None:
        self._separator_tag = separator_tag
        super().__init__(path, **kwargs)

    def read(self, path: str, **kwargs: Any) -> pa.Table:
        return _xmlstreamer.read_xml(
            str(path), separator_tag=self._separator_tag, **kwargs
        )

    def _iter_record_batches_stream(
        self,
        memory: Any = "64MiB",
        batch_size: Optional[int] = None,
        **plan_overrides: Any,
    ) -> Iterator[pa.RecordBatch]:
        plan = self._build_plan_kwargs()
        if plan_overrides:
            plan.update(plan_overrides)
        yield from _xmlstreamer.read_xml_stream(
            str(self._path),
            separator_tag=self._separator_tag,
            memory=memory,
            **plan,
        )
