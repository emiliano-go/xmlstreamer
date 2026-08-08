"""Command line: stream an XML feed and print one JSON object per item.

    uvx xmlstreamer <url> [--tag item] [--mode temp|stream] [--limit N]
"""

import argparse
import json
import os
import sys

from itertools import islice

from typing import List
from typing import Optional

from . import (
    FeedInterruptedError,
    Nested,
    StreamInterpreter,
    Transport,
    __version__,
)


def nonnegative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be zero or positive")
    return parsed


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="xmlstreamer",
        description="Stream an XML feed and print one JSON object per item.",
    )
    parser.add_argument("url", help="feed URL (http, https or ftp)")
    parser.add_argument(
        "-t", "--tag", default="item",
        help="separator tag that delimits items (default: item)",
    )
    parser.add_argument(
        "--mode", choices=("temp", "stream"), default="temp",
        help="download mode: spool to a temp file or parse straight "
        "from the network (default: temp)",
    )
    parser.add_argument(
        "--limit", type=nonnegative_int, default=None, metavar="N",
        help="stop after N items and release the source",
    )
    parser.add_argument(
        "--nested", action="store_true",
        help="emit nested dicts instead of flat path keys",
    )
    parser.add_argument(
        "--max-seconds", type=int, default=None, metavar="S",
        help="per-run time budget in seconds",
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true",
        help="log progress and stats to stderr",
    )
    parser.add_argument(
        "--version", action="version", version=f"xmlstreamer {__version__}"
    )
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    if args.verbose:
        import logging

        logging.basicConfig(stream=sys.stderr, level=logging.DEBUG)

    try:
        # Construction validates arguments: keep it inside the handler
        # so a bad flag prints one line instead of a traceback.
        interpreter = StreamInterpreter(
            url=args.url,
            separator_tag=args.tag,
            max_running_time=args.max_seconds,
            output=Nested() if args.nested else None,
            transport=Transport(download_mode=args.mode),
        )
        with interpreter:
            # islice pulls exactly N items: the for-then-break pattern
            # would consume one extra item just to discard it. Never
            # call iter() here: each call restarts the download.
            items = (
                interpreter if args.limit is None
                else islice(interpreter, args.limit)
            )
            for item in items:
                print(json.dumps(item, ensure_ascii=False))
    except FeedInterruptedError as exc:
        print(
            f"xmlstreamer: feed interrupted after {exc.items_delivered} "
            f"items: {exc.__cause__}",
            file=sys.stderr,
        )
        return 2
    except BrokenPipeError:
        # Downstream closed early (e.g. piped into head): exit quietly.
        devnull = os.open(os.devnull, os.O_WRONLY)
        os.dup2(devnull, sys.stdout.fileno())
        return 0
    except Exception as exc:
        # Network and setup failures: a clean one-liner, no traceback.
        print(f"xmlstreamer: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
