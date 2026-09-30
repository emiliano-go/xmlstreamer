# rypipe adapter benchmark

feed: /tmp/tmpcixfebd_.xml (128.1 MiB)

| engine | items | seconds | items/s | MB/s | peak RSS MB |
|---|---|---|---|---|---|
| legacy (per-item) | 953,250 | 6.970 | 136,759 | 18.4 | 201.2 |
| rypipe columnar | 953,250 | 4.156 | 229,393 | 30.8 | 264.9 |
| rypipe par4 | 953,250 | 0.558 | 1,709,854 | 229.7 | 278.3 |
| rypipe par8 | 953,250 | 0.660 | 1,444,426 | 194.1 | 278.7 |
| rypipe par12 | 953,250 | 0.647 | 1,473,380 | 198.0 | 279.1 |
| rypipe stream64 | 953,250 | 4.208 | 226,522 | 30.4 | 143.5 |
| rypipe par-stream8 | 953,250 | 0.828 | 1,151,630 | 154.7 | 141.5 |
| rypipe columnar drop4 | 953,250 | 3.317 | 287,397 | 38.6 | 209.5 |
