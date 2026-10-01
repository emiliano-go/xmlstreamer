# xmlstreamer benchmarks

Date: 2026-09-30 | CPU: 13th Gen Intel(R) Core(TM) i5-1335U | Python 3.13.13 | xmlstreamer 2.0.0, lxml 6.1.3, xmltodict 1.0.4, chardet 7.6.0

## Speed (parse-only, same in-memory bytes, best of N)

| feed | parser | items | seconds | items/s |
|---|---|---|---|---|
| plain | xmlstreamer | 100,000 | 0.382 | 261,698 |
| plain | lxml iterparse (recover=True) | 100,000 | 0.370 | 269,970 |
| plain | stdlib ElementTree iterparse | 100,000 | 0.391 | 255,714 |
| plain | xmltodict | 100,000 | 1.228 | 81,440 |
| plain | xmltodict (streaming mode) | 100,000 | 1.108 | 90,240 |
| cdata | xmlstreamer | 50,000 | 0.323 | 154,961 |
| cdata | lxml iterparse (recover=True) | 50,000 | 0.490 | 102,006 |
| cdata | stdlib ElementTree iterparse | 50,000 | 0.216 | 231,527 |
| cdata | xmltodict | 50,000 | 0.666 | 75,103 |
| cdata | xmltodict (streaming mode) | 50,000 | 0.644 | 77,580 |
| plain | xmlstreamer end-to-end (HTTP) | 100,000 | 0.417 | 240,091 |

## Peak memory (one subprocess per parser, RSS)

| parser | feed MB | peak RSS MB | items | seconds |
|---|---|---|---|---|
| (python + imports baseline) | 121 | 16 | 0 | 0.00 |
| xmlstreamer | 121 | 73 | 900,000 | 4.80 |
| lxml iterparse (recover=True) | 121 | 24 | 900,000 | 4.77 |
| stdlib ElementTree iterparse | 121 | 17 | 900,000 | 4.80 |
| xmltodict (streaming mode) | 121 | 22 | 900,000 | 11.71 |
| xmltodict | 121 | 443 | 900,000 | 11.46 |

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
