# xmlstreamer benchmarks

Date: 2026-09-30 | CPU: 13th Gen Intel(R) Core(TM) i5-1335U | Python 3.13.13 | xmlstreamer 2.0.0, lxml 6.1.3, xmltodict 1.0.4, chardet 7.6.0

## Speed (parse-only, same in-memory bytes, best of N)

| feed | parser | items | seconds | items/s |
|---|---|---|---|---|
| plain | xmlstreamer | 100,000 | 0.340 | 294,370 |
| plain | lxml iterparse (recover=True) | 100,000 | 0.394 | 253,525 |
| plain | stdlib ElementTree iterparse | 100,000 | 0.374 | 267,196 |
| plain | xmltodict | 100,000 | 1.123 | 89,080 |
| plain | xmltodict (streaming mode) | 100,000 | 1.101 | 90,796 |
| cdata | xmlstreamer | 50,000 | 0.215 | 232,412 |
| cdata | lxml iterparse (recover=True) | 50,000 | 0.288 | 173,328 |
| cdata | stdlib ElementTree iterparse | 50,000 | 0.222 | 225,182 |
| cdata | xmltodict | 50,000 | 0.675 | 74,021 |
| cdata | xmltodict (streaming mode) | 50,000 | 0.562 | 88,963 |
| plain | xmlstreamer end-to-end (HTTP) | 100,000 | 0.456 | 219,442 |

## Peak memory (one subprocess per parser, RSS)

| parser | feed MB | peak RSS MB | items | seconds |
|---|---|---|---|---|
| (python + imports baseline) | 121 | 16 | 0 | 0.00 |
| xmlstreamer | 121 | 73 | 900,000 | 3.80 |
| lxml iterparse (recover=True) | 121 | 24 | 900,000 | 3.97 |
| stdlib ElementTree iterparse | 121 | 17 | 900,000 | 3.59 |
| xmltodict (streaming mode) | 121 | 22 | 900,000 | 11.46 |
| xmltodict | 121 | 443 | 900,000 | 11.69 |

## Dirty-feed gauntlet (what each parser does to broken input)

| case | xmlstreamer | lxml (recover) | stdlib ET | xmltodict |
|---|---|---|---|---|
| healthy feed | OK | OK | OK | OK |
| malformed item between healthy ones | OK: drops broken loudly (warning logged) | FABRICATES: emits 3 where 2 are real | LOUD FAILURE: ParseError, feed lost (emitted 1 of 2 first) | LOUD FAILURE: ExpatError, feed lost (emitted 0 of 2 first) |
| raw ampersand and raw < in text | OK: drops broken loudly (warning logged) | FABRICATES: emits 2 where 1 are real | LOUD FAILURE: ParseError, feed lost (emitted 0 of 1 first) | LOUD FAILURE: ExpatError, feed lost (emitted 0 of 1 first) |
| declaration lies: says iso-8859-1, bytes are utf-8 | OK | SILENT CORRUPTION: {'t': 'coraÃ§Ã£o'} instead of {'t': 'coração'} | SILENT CORRUPTION: {'t': 'coraÃ§Ã£o'} instead of {'t': 'coração'} | SILENT CORRUPTION: {'t': 'coraÃ§Ã£o'} instead of {'t': 'coração'} |
| declaration lies: says utf-8, bytes are latin-1 | OK | SILENT CORRUPTION: {'t': 'programaci�n regi�n caf�'} instead of {'t': 'programación región café'} | LOUD FAILURE: ParseError, feed lost (emitted 0 of 5 first) | LOUD FAILURE: ExpatError, feed lost (emitted 0 of 5 first) |
| CDATA with literal </item> plus commented-out item | OK | OK | OK | OK |
| download truncated mid-item | OK | FABRICATES: emits 3 where 2 are real | LOUD FAILURE: ParseError, feed lost (emitted 2 of 2 first) | LOUD FAILURE: ExpatError, feed lost (emitted 0 of 2 first) |
| nested separator (out of xmlstreamer's contract) | unsupported (documented contract) | parses it (emits 2) | parses it (emits 2) | parses it (emits 1) |

- malformed item between healthy ones: item b never closes its tag
- raw ampersand and raw < in text: only the clean item is deliverable without guessing
- download truncated mid-item: the third item never arrived; inventing it is corruption
- nested separator (out of xmlstreamer's contract): xmlstreamer documents this as unsupported
