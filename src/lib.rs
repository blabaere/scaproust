// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

//! Scaproust is an implementation of the [nanomsg](http://nanomsg.org/index.html) 
//! "Scalability Protocols" in the [Rust programming language](http://www.rust-lang.org/).
//!
//! # Goals
//!
//! * Support for all of nanomsg's protocols.
//! * Support for TCP and IPC transports.
//! * Idiomatic rust API first, mimic the original C API second.
//! * Extensibility: allow user code to define additional protocols and transports
//!
//! # Usage
//!
//! First, build a [Session](struct.Session.html) with the transports you need 
//! (this will start the thread performing the actual I/O operations).  
//! Then, use the session to create some [Socket](struct.Socket.html), 
//! specifying the communication pattern.  
//! If you want, you can now [set some options](struct.Socket.html#method.set_option), like the timeouts.  
//! To plug the sockets, use the [connect](struct.Socket.html#method.connect) and [bind](struct.Socket.html#method.bind) socket methods.  
//! Finally, use the socket methods [send](struct.Socket.html#method.send) and
//! [recv](struct.Socket.html#method.recv) to exchange messages between sockets.  
//! When in doubts, please refer to the [nanomsg manual](http://nanomsg.org/v1.0.0/nanomsg.7.html).  
//!
//! # Example
//!
//! ```
//! use scaproust::*;
//! use std::time::Duration;
//! 
//! let mut session = SessionBuilder::new().with("tcp", Tcp).build().unwrap();
//! let mut pull = session.create_socket::<Pull>().unwrap();
//! let mut push = session.create_socket::<Push>().unwrap();
//! let timeout = Duration::from_millis(250);
//! 
//! pull.set_recv_timeout(Some(timeout)).unwrap();
//! pull.bind("tcp://127.0.0.1:5454").unwrap();
//! 
//! push.set_send_timeout(Some(timeout)).unwrap();
//! push.connect("tcp://127.0.0.1:5454").unwrap();
//! 
//! push.send(vec![65, 66, 67]).unwrap();
//! let received = pull.recv().unwrap();
//! ```

#![crate_name = "scaproust"]
#![doc(html_root_url = "https://blabaere.github.io/scaproust/")]


#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(stmt_expr_attributes)]
#![feature(conservative_impl_trait)]

#[cfg_attr(feature = "cargo-clippy", allow(boxed_local))]
#[cfg_attr(feature = "cargo-clippy", allow(bool_comparison))]
#[cfg_attr(feature = "cargo-clippy", allow(len_without_is_empty))]

#[macro_use]
extern crate log;
extern crate time;
extern crate byteorder;
extern crate mio;
extern crate mio_uds;

#[cfg(windows)]
extern crate mio_named_pipes;
#[cfg(windows)]
extern crate winapi;

pub mod core;
pub mod proto;
pub mod transport;

#[doc(hidden)]
mod reactor;
#[doc(hidden)]
mod facade;

#[doc(hidden)]
mod sequence;
#[doc(hidden)]
mod io_error;
#[doc(hidden)]
mod sync;

pub use facade::session::SessionBuilder;
pub use facade::session::Session;
pub use facade::socket::Socket;
pub use facade::device::Device;
pub use facade::probe::Probe;
pub use facade::endpoint::Endpoint;
pub use core::Message;
pub use core::PollReq;
pub use core::PollRes;
pub use core::config::ConfigOption;

pub use transport::tcp::Tcp;
pub use transport::ipc::Ipc;

pub use proto::pair::Pair;
pub use proto::publ::Pub;
pub use proto::sub::Sub;
pub use proto::req::Req;
pub use proto::rep::Rep;
pub use proto::push::Push;
pub use proto::pull::Pull;
pub use proto::surv::Surveyor;
pub use proto::resp::Respondent;
pub use proto::bus::Bus;

#[cfg(test)]
mod tests {
    struct Editable {
        x: usize
    }
    struct Editor {
        y: usize
    }
    struct Outter {
        editable: Editable,
        editor: Editor
    }
    impl Editable {
        fn edit(&mut self) { self.x += 1; }
    }
    impl Editor {
        fn edit(&mut self, editable: &mut Editable) { 
            self.y += 1; 
            editable.edit();
        }
    }
    impl Outter {
        fn test(&mut self) {
            self.editor.edit(&mut self.editable);
        }
    }

    #[test]
    fn can_pass_mutable_field_ref_to_other_field() {
        let mut master = Outter {
            editable: Editable { x: 0 },
            editor: Editor { y: 0 },
        };

        master.test();
    }
}