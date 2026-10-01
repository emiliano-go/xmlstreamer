# rypipe adapter benchmark

feed: /tmp/tmpz3on23sc.xml (128.1 MiB)

| engine | items | seconds | items/s | MB/s | peak RSS MB |
|---|---|---|---|---|---|
| legacy (per-item) | 953,250 | 4.477 | 212,923 | 28.6 | 201.7 |
| rypipe columnar | 953,250 | 1.220 | 781,462 | 105.0 | 309.5 |
| rypipe par4 | 953,250 | 0.320 | 2,980,502 | 400.5 | 282.9 |
| rypipe par8 | 953,250 | 0.321 | 2,971,675 | 399.3 | 283.4 |
| rypipe par12 | 953,250 | 0.331 | 2,883,838 | 387.5 | 283.0 |
| rypipe stream64 | 953,250 | 1.530 | 623,155 | 83.7 | 145.8 |
| rypipe par-stream8 | 953,250 | 0.396 | 2,405,662 | 323.2 | 146.0 |
| rypipe columnar drop4 | 953,250 | 0.769 | 1,238,995 | 166.5 | 216.7 |
| rypipe columnar proj1 | 953,250 | 0.887 | 1,074,241 | 144.3 | 217.1 |
| rypipe par8 proj1 | 953,250 | 0.161 | 5,908,620 | 793.9 | 212.9 |
