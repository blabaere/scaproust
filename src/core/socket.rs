// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;
use std::fmt;

use message::Message;
use core::protocol::Protocol;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct SocketId(usize);

impl fmt::Debug for SocketId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for SocketId {
    fn from(value: usize) -> SocketId {
        SocketId(value)
    }
}

pub enum Request {
    Connect(String),
    Bind(String),
    Send(Message),
    Recv,
    SetOption,
}

pub enum Reply {
    Connect(io::Result<()>),
    Bind(io::Result<()>),
    Send(io::Result<()>),
    Recv(io::Result<Message>),
    SetOption(io::Result<()>)
}

pub struct Socket {
    reply_sender: Sender<Reply>,
    protocol: Box<Protocol>
}

impl Socket {
    pub fn new(reply_tx: Sender<Reply>, proto: Box<Protocol>) -> Socket {
        Socket {
            reply_sender: reply_tx,
            protocol: proto
        }
    }
}

pub struct SocketCollection {
    id_sequence: usize,
    sockets: HashMap<SocketId, Socket>
}

impl SocketCollection {
    pub fn new() -> SocketCollection {
        SocketCollection {
            id_sequence: 0,
            sockets: HashMap::new()
        }
    }

    pub fn add(&mut self, reply_tx: Sender<Reply>, proto: Box<Protocol>) -> SocketId {
        let id = SocketId(self.id_sequence);
        let socket = Socket::new(reply_tx, proto);

        self.sockets.insert(id, socket);
        self.id_sequence += 1;

        id
    }
}
