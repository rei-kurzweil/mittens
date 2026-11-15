Scenario: vr2. CPU times cover the whole run, including the drain interval.

| Participants | Mode | Replay ns/packet, median [min–max] | Receiver CPU ms / whole run, median [min–max] | UDP p95 µs, median | Drops / send errors | Scratch bytes |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | runtime | — | 1.554 [1.408–1.616] | 11122.5 | 0 / 0 | 65536 |
| 2 | separate | 84.448 [83.437–91.677] | 1.499 [1.415–1.623] | 11117.9 | 0 / 0 | 132096 |
| 2 | shared | 86.306 [83.446–86.668] | 1.558 [1.514–1.604] | 11125.0 | 0 / 0 | 65536 |

Shared minus separate time; negative means shared took less time:
  pairs=2 replay: paired median +1.15%, range [-5.86%, +2.54%], n=4
  pairs=2 udp: paired median +4.01%, range [-3.57%, +9.54%], n=4
