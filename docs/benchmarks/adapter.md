# rypipe adapter benchmark

feed: /tmp/tmpnv_6fmc1.xml (128.1 MiB)

| engine | items | seconds | items/s | MB/s | peak RSS MB |
|---|---|---|---|---|---|
| legacy (per-item) | 953,250 | 3.563 | 267,511 | 35.9 | 202.1 |
| rypipe columnar | 953,250 | 1.316 | 724,452 | 97.3 | 309.9 |
| rypipe par4 | 953,250 | 0.285 | 3,343,552 | 449.2 | 283.3 |
| rypipe par8 | 953,250 | 0.280 | 3,402,566 | 457.2 | 283.2 |
| rypipe par12 | 953,250 | 0.281 | 3,394,282 | 456.0 | 283.2 |
| rypipe stream64 | 953,250 | 1.436 | 663,982 | 89.2 | 146.1 |
| rypipe par-stream8 | 953,250 | 0.360 | 2,648,807 | 355.9 | 146.4 |
| rypipe columnar drop4 | 953,250 | 0.593 | 1,607,930 | 216.0 | 217.3 |
| rypipe columnar proj1 | 953,250 | 0.589 | 1,617,310 | 217.3 | 217.0 |
| rypipe par8 proj1 | 953,250 | 0.151 | 6,310,285 | 847.8 | 213.3 |
