Scenario: vr4. CPU times cover the whole run, including the drain interval.

| Participants | Mode | Replay ns/packet, median [min–max] | Receiver CPU ms / whole run, median [min–max] | UDP p95 µs, median | Drops / send errors | Scratch bytes |
| --- | --- | --- | --- | --- | --- | --- |
| 4 | runtime | — | 2.010 [1.954–2.193] | 11122.2 | 0 / 0 | 65536 |
| 4 | separate | 93.492 [92.131–103.293] | 1.977 [1.856–2.043] | 11111.2 | 0 / 0 | 132096 |
| 4 | shared | 99.889 [98.107–106.638] | 1.974 [1.885–2.091] | 11119.1 | 0 / 0 | 65536 |

Shared minus separate time; negative means shared took less time:
  pairs=4 replay: paired median +6.35%, range [+3.24%, +7.47%], n=4
  pairs=4 udp: paired median +1.22%, range [-2.11%, +3.29%], n=4
