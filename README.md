# scaproust <img src=albertine-like.jpg align=right width=300 height=300>

Scaproust is an implementation of the [nanomsg](http://nanomsg.org/index.html) "Scalability Protocols" in rust.

**Experimental work !** For working stuff, please see [nanomsg-rs](https://github.com/blabaere/nanomsg.rs).  
Scaproust is internally based on [mio](https://github.com/carllerche/mio), so MS Windows is not yet supported.

## Goals
* Support for all of nanomsg's protocols.
* Support for TCP and IPC transports.
* Idiomatic rust API first, mimic the original CAPI second.
* Zero-copy, minimal allocations.

## Non goals
* Ability to use a socket as a raw file descriptor with system level functions.

## Maybe
* Polling on several sockets.
* Low-latency (current design use channels between user facing functions and system functions).
* Other transports (Inproc, TLS, WebSockets).
* Async API, using future/promise to represent send/recv results.