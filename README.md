# scaproust <img src=albertine-like.jpg align=right width=300 height=300>

Scaproust is an implementation of the [nanomsg](http://nanomsg.org/index.html) "Scalability Protocols" in rust.

**Experimental work !** For a working piece of software, please see [nanomsg-rs](https://github.com/blabaere/nanomsg.rs).

## Goals
* Cover all of nanomsg's protocols
* Idiomatic rust API
* Zero-copy

## Non goals
* Ability to use a socket as a raw file descriptor with system level functions.

## Maybe in future
* Polling on several socket
* Low-latency (current design use channels between user facing functions and system functions).
