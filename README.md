# Scaproust - Scalability Protocols in Rust

[![Linux build](https://travis-ci.org/blabaere/scaproust.svg?label=linux)](https://travis-ci.org/blabaere/scaproust)
[![Windows build](https://ci.appveyor.com/api/projects/status/kpqdm42mhlki39fq?svg=true)](https://ci.appveyor.com/project/blabaere/scaproust)
[![crates.io](http://meritbadge.herokuapp.com/scaproust)](https://crates.io/crates/scaproust)

Scaproust is an implementation of the [nanomsg](http://nanomsg.org/index.html) "Scalability Protocols" in the [Rust programming language](http://www.rust-lang.org/).

Quoting from nanomsg's site:
> nanomsg is a socket library that provides several common communication patterns. It aims to make the networking layer fast, scalable, and easy to use. Implemented in C, it works on a wide range of operating systems with no further dependencies.

> The communication patterns, also called "scalability protocols", are basic blocks for building distributed systems. By combining them you can create a vast array of distributed applications. 

**Experimental work !** For working stuff, please see [nanomsg-rs](https://github.com/blabaere/nanomsg.rs).  

[API Documentation](https://blabaere.github.io/scaproust/scaproust/index.html)

## Goals
* Support for all of nanomsg's protocols.
* Support for TCP and IPC transports.
* Idiomatic rust API first, mimic the original C API second.
* Extensibility: allow user code to define additional protocols and transports

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
scaproust = "0.1.0"
```

Next, add this to your crate:

```rust
extern crate scaproust;
```

## Progress
- [ ] Protocols
  - [x] PAIR
  - [x] BUS
  - [ ] REQREP
    - [x] REQ
    - [x] REQ resend
    - [ ] REQ prefetch replies
    - [x] REP
  - [x] PUBSUB
    - [x] PUB
    - [x] SUB
    - [x] SUB subscription filter
  - [x] PIPELINE
    - [x] PUSH
    - [x] PULL
  - [x] SURVEY
    - [x] SURVEYOR
    - [x] SURVEYOR deadline
    - [x] RESPONDENT  

- [ ] Transports
  - [x] TCP
  - [x] IPC (*nix)
  - [ ] IPC (Windows)

- [ ] Socket options
  - [ ] Linger
  - [ ] Recv max size
  - [x] Send timeout
  - [x] Recv timeout
  - [x] Reconnect interval
  - [ ] Reconnect interval max
  - [x] Send priority
  - [x] Recv priority
  - [ ] IPV4 only
  - [ ] Socket name

- [x] Protocol options
  - [x] REQ resend interval
  - [x] SURVEYOR deadline
  - [x] SUB subscribe
  - [x] SUB unsubscribe

- [ ] Transport options
  - [ ] TCP no delay

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed as above, without any
additional terms or conditions.