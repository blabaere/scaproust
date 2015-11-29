// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

#![crate_name = "scaproust"]

#![feature(drain)]
#![feature(fnbox)]
#![feature(unboxed_closures)]

#[macro_use] extern crate log;
extern crate byteorder;
extern crate mio;
extern crate time;

mod global;
mod event_loop_msg;
mod facade;
mod session;
mod socket;
mod protocol;
mod transport;
mod endpoint;
mod pipe;
mod send;
mod acceptor;

use std::boxed::FnBox;

pub use facade::{
    SessionFacade as Session,
    SocketFacade as Socket
};

pub use global::SocketType;
pub use event_loop_msg::SocketOption;

type EventLoop = mio::EventLoop<session::Session>;
type EventLoopAction = Box<FnBox(&mut EventLoop) -> bool>;

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