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

    __slots__ = ("_separator_tag", "_engine_mode", "_threads")

    def __init__(
        self,
        path,
        *,
        separator_tag: str = "item",
        engine: str = "columnar",
        threads: Optional[int] = None,
        **kwargs: Any,
    ) -> None:
        self._separator_tag = separator_tag
        self._engine_mode = engine
        self._threads = threads
        super().__init__(path, **kwargs)

    def _resolve_mode(self, path: str, mode: str, threads: Optional[int]) -> str:
        if mode != "auto":
            return mode
        import os

        from rypipe import resolve_engine

        resolved = resolve_engine(
            file_size=os.path.getsize(path),
            threads=threads,
            schema=self._schema or None,
            has_parallel=True,
            has_columnar=True,
        )
        # `read` returns a table, so the streaming modes map to parallel.
        return "parallel" if resolved in ("parallel", "parallel_streaming") else "columnar"

    def read(self, path: str, **kwargs: Any) -> pa.Table:
        mode = kwargs.pop("engine", self._engine_mode)
        threads = kwargs.pop("threads", self._threads)
        chunks = kwargs.pop("chunks", None)
        mode = self._resolve_mode(path, mode, threads)
        if mode == "parallel" or (mode == "columnar" and threads and threads > 1):
            return _xmlstreamer.read_xml_par(
                str(path),
                separator_tag=self._separator_tag,
                chunks=chunks or threads or 8,
                **kwargs,
            )
        return _xmlstreamer.read_xml(
            str(path), separator_tag=self._separator_tag, **kwargs
        )

    def _iter_record_batches_stream(
        self,
        memory: Any = "64MiB",
        batch_size: Optional[int] = None,
        **plan_overrides: Any,
    ) -> Iterator[pa.RecordBatch]:
        # Parallel streaming is the default when threads are requested; it
        # preserves file order and bounds parser memory. Without threads the
        # sequential bounded path is used.
        threads = plan_overrides.pop("threads", None)
        ordered = plan_overrides.pop("ordered", True)
        plan = self._build_plan_kwargs()
        if plan_overrides:
            plan.update(plan_overrides)
        if threads and threads > 1:
            yield from _xmlstreamer.iter_xml_batches_par(
                str(self._path),
                separator_tag=self._separator_tag,
                threads=threads,
                memory=memory,
                ordered=ordered,
                **plan,
            )
            return
        yield from _xmlstreamer.read_xml_stream(
            str(self._path),
            separator_tag=self._separator_tag,
            memory=memory,
            **plan,
        )
