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
use super::protocol::Protocol;
use super::endpoint::Endpoint;
use super::endpoint::EndpointId;
use super::network::Network;

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
    Err(io::Error),
    Connect(EndpointId),
    Bind(EndpointId),
    Send,
    Recv(Message),
    SetOption
}

pub struct Socket {
    id: SocketId,
    reply_sender: Sender<Reply>,
    protocol: Box<Protocol>,
    remote_endpoints: HashMap<EndpointId, Endpoint>,
    local_endpoints: HashMap<EndpointId, Endpoint>
}

impl Socket {
    pub fn new(id: SocketId, reply_tx: Sender<Reply>, proto: Box<Protocol>) -> Socket {
        Socket {
            id: id,
            reply_sender: reply_tx,
            protocol: proto,
            remote_endpoints: HashMap::new(),
            local_endpoints: HashMap::new(), 
        }
    }

    pub fn connect(&mut self, network: &Network, url: String) {
        network.connect(self.id, &url).
            map(|eid| self.connect_succeeded(eid, url)).
            map_err(|err| self.connect_failed(err)).
            unwrap()
    }

    fn connect_succeeded(&mut self, eid: EndpointId, url: String) {
        let ep = Endpoint::new_created(eid, url);

        self.remote_endpoints.insert(eid, ep);
        self.reply_sender.send(Reply::Connect(eid));
    }

    fn connect_failed(&mut self, err: io::Error) {
        self.reply_sender.send(Reply::Err(err));
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
        let socket = Socket::new(id, reply_tx, proto);

        self.sockets.insert(id, socket);
        self.id_sequence += 1;

        id
    }

    pub fn do_on_socket<F>(&self, id: SocketId, f: F) where F : FnOnce(&Socket) {
        if let Some(socket) = self.sockets.get(&id) {
            f(socket)
        }
    }

    pub fn do_on_socket_mut<F>(&mut self, id: SocketId, f: F) where F : FnOnce(&mut Socket) {
        if let Some(socket) = self.sockets.get_mut(&id) {
            f(socket)
        }
    }
}

