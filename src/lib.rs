// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

#![crate_name = "scaproust"]
#![doc(html_root_url = "https://blabaere.github.io/scaproust/")]

#![feature(box_syntax)]
#![feature(unboxed_closures)]
#![feature(fnbox)]
//#![feature(plugin)]
//#![plugin(clippy)]
//#![allow(boxed_local)]
//#![allow(bool_comparison)]
//#![allow(explicit_iter_loop)]

#[macro_use]
extern crate log;
extern crate byteorder;
extern crate mio;
extern crate time;

mod message;
mod core;
mod ctrl;
mod facade;
mod util;

pub use facade::session::Session as Session;
pub use facade::socket::Socket as Socket;
pub use core::protocol::Protocol as Protocol;