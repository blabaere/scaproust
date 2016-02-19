// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

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
