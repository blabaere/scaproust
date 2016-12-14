
### Average latency (µs)
| Msg Size | Roundtrips | Nanomsg | Scaproust |
| --- | --- | --- | --- |
| 512 | 50000 | 19 | 26 |
| 1024 | 10000 | 21 | 27 |
| 8192 | 10000 | 23 | 29 |
| 102400 | 2000 | 56 | 61 |
| 524288 | 500 | 323 | 216 |
| 1048576 | 100 | 794 | 782 |

Potential scaproust optimization places:
- Message allocations, send side
- Message allocations, receive side
- Event loop polling
- Synchronization in the downstream mio channel
- Synchronization in the upstream std channel
- Incoming messages are not fetched until the user code requests it
- ???