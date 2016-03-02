// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
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
//!
//! # Usage
//!
//! First, create a [Session](struct.Session.html) 
//! (this will start the thread performing the actual I/O operations).  
//! Then, use the session to create some [Socket](struct.Socket.html), 
//! specifying the communication pattern with [SocketType](enum.SocketType.html).  
//! If you want, you can now [set some options](struct.Socket.html#method.set_option), like the timeouts.  
//! To plug the sockets, use the [connect](struct.Socket.html#method.connect) and [bind](struct.Socket.html#method.bind) socket methods.  
//! Finally, use the socket methods [send](struct.Socket.html#method.send) and
//! [recv](struct.Socket.html#method.recv) to exchange messages between sockets.  
//! When in doubts, refer to the [nanomsg documentation](http://nanomsg.org/v0.8/nanomsg.7.html).  
//!
//! # Example
//!
//! ```
//! use scaproust::*;
//! use std::time;
//! 
//! let session = Session::new().unwrap();
//! let mut pull = session.create_socket(SocketType::Pull).unwrap();
//! let mut push = session.create_socket(SocketType::Push).unwrap();
//! let timeout = time::Duration::from_millis(250);
//! 
//! push.set_recv_timeout(timeout).unwrap();
//! pull.bind("tcp://127.0.0.1:5454").unwrap();
//! 
//! push.set_send_timeout(timeout).unwrap();
//! push.connect("tcp://127.0.0.1:5454").unwrap();
//! 
//! push.send(vec![65, 66, 67]).unwrap();
//! let received = pull.recv().unwrap();
//! ```


#![crate_name = "scaproust"]
#![doc(html_root_url = "https://blabaere.github.io/scaproust/")]

#![feature(box_syntax)]

#[macro_use]
extern crate log;
extern crate byteorder;
extern crate mio;
extern crate time;

mod global;
mod event_loop_msg;
mod session_facade;
mod socket_facade;
mod device_facade;
mod endpoint_facade;
mod session;
mod socket;
mod protocol;
mod transport;
mod pipe;
mod send;
mod recv;
mod acceptor;
mod probe;

pub use session_facade::SessionFacade as Session;
pub use socket_facade::SocketFacade as Socket;
pub use device_facade::DeviceFacade as Device;
pub use endpoint_facade::EndpointFacade as Endpoint;

pub use global::SocketType;
pub use event_loop_msg::SocketOption;

pub type EventLoop = mio::EventLoop<session::Session>;

pub struct Message {
    pub header: Vec<u8>,
    pub body: Vec<u8>
}

impl Message {
    pub fn with_body(buffer: Vec<u8>) -> Message {
        Message {
            header: Vec::new(),
            body: buffer
        }
    }

    pub fn with_header_and_body(header: Vec<u8>, buffer: Vec<u8>) -> Message {
        Message { header: header, body: buffer }
    }

    pub fn len(&self) -> usize {
        self.header.len() + self.body.len()
    }

    pub fn get_header<'a>(&'a self) -> &'a [u8] {
        &self.header
    }

    pub fn get_body<'a>(&'a self) -> &'a [u8] {
        &self.body
    }

    pub fn to_buffer(self) -> Vec<u8> {
        self.body
    }

    pub fn explode(self) -> (Vec<u8>, Vec<u8>) {
        (self.header, self.body)
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        Message::with_header_and_body(self.header.clone(), self.body.clone())
    }
}
