# scaproust <img src=albertine-like.jpg align=right width=200 height=200>

Scaproust is an implementation of the [nanomsg](http://nanomsg.org/index.html) "Scalability Protocols" in rust.

**Experimental work !** For working stuff, please see [nanomsg-rs](https://github.com/blabaere/nanomsg.rs).  
Scaproust is internally based on [mio](https://github.com/carllerche/mio), so MS Windows is not yet supported.

## Goals
* Support for all of nanomsg's protocols.
* Support for TCP and IPC transports.
* Idiomatic rust API first, mimic the original C API second.

## Maybe
* Zero-copy, minimal allocations.
* Polling on several sockets.
* Low-latency (current design use channels between user facing functions and system functions).
* Other transports (Inproc, TLS, WebSockets).
* Async API, using future/promise to represent send/recv results.
* Efficient nonblocking operations (difficult due to the aboce mentioned use of channels).

## Non goals
* Ability to use a socket as a raw file descriptor with system level functions.

## Progress
- [ ] Protocols
  - [x] PAIR
  - [x] BUS
  - [ ] REQREP
    - [x] REQ
    - [ ] REQ resend
    - [x] REP
  - [ ] PUBSUB
    - [x] PUB
    - [x] SUB
    - [ ] SUB subscription filter
  - [x] PIPELINE
    - [x] PUSH
    - [x] PULL
  - [ ] SURVEY
    - [ ] SURVEYOR
    - [x] RESPONDENT  

- [ ] Transports
  - [x] TCP
  - [ ] IPC
  - [ ] INPROC  

- [ ] Basic features
  - [x] Send (buffer only)
  - [x] Recv (buffer only)
  - [x] Connect 
  - [x] Reconnect on failure
  - [x] Bind
  - [x] Rebind on failure
  - [ ] Device
  - [ ] Logs
  - [ ] Statistics

- [ ] Advanced features
  - [ ] Send (scatter array + control header)
  - [ ] Recv (scatter array + control header)
  - [ ] Fair queuing
  - [ ] Load balancing
  - [ ] Send priority
  - [ ] Recv priority

- [ ] Socket options
  - [ ] Linger
  - [ ] Send buffer size
  - [ ] Recv buffer size
  - [ ] Send timeout
  - [ ] Recv timeout
  - [ ] Reconnect interval
  - [ ] Reconnect interval max
  - [ ] Send priority
  - [ ] Recv priority
  - [ ] IPV4 only
  - [ ] Socket name

- [ ] Protocol options
    - [ ] REQ resend interval
    - [ ] SURVEYOR deadline
    - [ ] SUB subscribe
    - [ ] SUB unsubscribe

- [ ] Transport options
    - [ ] TCP no delay
