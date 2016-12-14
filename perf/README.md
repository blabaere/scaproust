## Performance comparaison of scaproust VS nanomsg
The comparaison is made using the perf utilities shipped with nanomsg and their scaproust counterpart.
This is probably not of a very subtle benchmark, but given how far the results are, it is still useful.

### Average latency (µs)
| Msg Size | Roundtrips | Nanomsg | Scaproust |
| ---: | ---: | ---: | ---: |
| 512 | 50000 | 19 | 26 |
| 1024 | 10000 | 21 | 27 |
| 8192 | 10000 | 23 | 29 |
| 102400 | 2000 | 56 | 61 |
| 524288 | 500 | 323 | 216 |
| 1048576 | 100 | 794 | 782 |

### Average throughput (Mb/s)
| Msg Size | Msg Count | Nanomsg | Scaproust |
| ---: | ---: | ---: | ---: |
| 512 | 1000000 | 3091 | 312 |
| 1024 | 500000 | 5511 | 605 |
| 8192 | 50000 | 13865 | 3700 |
| 131072 | 10000 | 19694 | 18798 |
| 524288 | 2000 | 16215 | 20708 |
| 1048576 | 1000 | 12501 | 11733 |

## Potential scaproust optimization places
- Message allocations, send side
- Message allocations, receive side
- Event loop polling
- Synchronization in the downstream mio channel
- Synchronization in the upstream std channel
- Incoming messages are not fetched until the user code requests it
- Registering the stream each time an operation completes is inefficient ?
- ???