# rypipe adapter benchmark

feed: /tmp/tmpn4fabhcv.xml (128.1 MiB)

| engine | items | seconds | items/s | MB/s | peak RSS MB |
|---|---|---|---|---|---|
| legacy (per-item) | 953,250 | 4.721 | 201,914 | 27.1 | 201.7 |
| rypipe columnar | 953,250 | 1.350 | 706,355 | 94.9 | 290.0 |
| rypipe par4 | 953,250 | 0.337 | 2,830,386 | 380.3 | 283.2 |
| rypipe par8 | 953,250 | 0.329 | 2,899,682 | 389.6 | 283.1 |
| rypipe par12 | 953,250 | 0.320 | 2,974,565 | 399.7 | 283.4 |
| rypipe stream64 | 953,250 | 1.508 | 632,219 | 84.9 | 145.7 |
| rypipe par-stream8 | 953,250 | 0.395 | 2,414,572 | 324.4 | 145.8 |
| rypipe columnar drop4 | 953,250 | 0.789 | 1,207,954 | 162.3 | 217.0 |
| rypipe columnar proj1 | 953,250 | 0.777 | 1,226,149 | 164.7 | 217.3 |
| rypipe par8 proj1 | 953,250 | 0.175 | 5,437,093 | 730.5 | 213.1 |
