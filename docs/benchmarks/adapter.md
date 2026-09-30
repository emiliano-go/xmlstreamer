# rypipe adapter benchmark

feed: /tmp/tmp4jc4k_9a.xml (128.1 MiB)

| engine | items | seconds | items/s | MB/s | peak RSS MB |
|---|---|---|---|---|---|
| legacy (per-item) | 953,250 | 3.433 | 277,677 | 37.3 | 201.7 |
| rypipe columnar | 953,250 | 1.109 | 859,616 | 115.5 | 310.0 |
| rypipe par4 | 953,250 | 0.302 | 3,151,957 | 423.5 | 282.9 |
| rypipe par8 | 953,250 | 0.314 | 3,040,053 | 408.5 | 283.0 |
| rypipe par12 | 953,250 | 0.306 | 3,115,871 | 418.6 | 282.9 |
| rypipe stream64 | 953,250 | 1.435 | 664,070 | 89.2 | 145.9 |
| rypipe par-stream8 | 953,250 | 0.358 | 2,659,793 | 357.4 | 145.8 |
| rypipe columnar drop4 | 953,250 | 0.755 | 1,263,042 | 169.7 | 217.0 |
| rypipe columnar proj1 | 953,250 | 0.676 | 1,410,507 | 189.5 | 217.1 |
| rypipe par8 proj1 | 953,250 | 0.161 | 5,917,796 | 795.1 | 212.8 |
