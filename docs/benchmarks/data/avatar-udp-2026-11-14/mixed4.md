Scenario: mixed4. CPU times cover the whole run, including the drain interval.

| Participants | Mode | Replay ns/packet, median [min–max] | Receiver CPU ms / whole run, median [min–max] | UDP p95 µs, median | Drops / send errors | Scratch bytes |
| --- | --- | --- | --- | --- | --- | --- |
| 4 | runtime | — | 1.954 [1.903–2.118] | 11124.6 | 0 / 0 | 65536 |
| 4 | separate | 100.206 [98.916–102.331] | 1.893 [1.721–1.989] | 11104.0 | 0 / 0 | 132096 |
| 4 | shared | 99.356 [97.781–115.638] | 1.871 [1.722–1.935] | 11115.6 | 0 / 0 | 65536 |

Shared minus separate time; negative means shared took less time:
  pairs=4 replay: paired median -1.57%, range [-3.03%, +16.91%], n=4
  pairs=4 udp: paired median -3.64%, range [-6.67%, +9.95%], n=4
