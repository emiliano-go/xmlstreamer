# rypipe adapter benchmark

feed: /tmp/tmp9wa3czmn.xml (128.1 MiB)

| engine | items | seconds | items/s | MB/s | peak RSS MB |
|---|---|---|---|---|---|
| legacy (per-item) | 953,250 | 4.437 | 214,822 | 28.9 | 201.7 |
| rypipe columnar | 953,250 | 1.176 | 810,552 | 108.9 | 309.9 |
| rypipe par4 | 953,250 | 0.326 | 2,921,182 | 392.5 | 282.8 |
| rypipe par8 | 953,250 | 0.308 | 3,090,731 | 415.3 | 282.8 |
| rypipe par12 | 953,250 | 0.309 | 3,082,969 | 414.2 | 282.9 |
| rypipe stream64 | 953,250 | 1.380 | 690,917 | 92.8 | 145.9 |
| rypipe par-stream8 | 953,250 | 0.394 | 2,421,039 | 325.3 | 146.3 |
| rypipe columnar drop4 | 953,250 | 0.783 | 1,217,308 | 163.6 | 217.2 |
| rypipe columnar typed | 953,250 | 1.576 | 604,664 | 81.2 | 312.3 |
| rypipe par8 typed | 953,250 | 0.362 | 2,629,689 | 353.3 | 279.3 |
