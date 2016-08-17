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
    protocol: Box<Protocol>
}

impl Socket {
    pub fn new(id: SocketId, reply_tx: Sender<Reply>, proto: Box<Protocol>) -> Socket {
        Socket {
            id: id,
            reply_sender: reply_tx,
            protocol: proto
        }
    }

    pub fn connect(&mut self, network: &Network, url: String) {
        let reply = match network.connect(self.id, &url) {
            Ok(id) => Reply::Connect(id),
            Err(e) => Reply::Err(e)
        };

        self.reply_sender.send(reply);
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::io;

    use super::*;
    use core::protocol::Protocol;
    use core::network::Network;
    use core::endpoint::EndpointId;
    use util;

    struct TestProto;

    impl Protocol for TestProto {
        fn do_it_bob(&self) -> u8 {0}
    }

    struct FailingNetwork;

    impl Network for FailingNetwork {
        fn connect(&self, socket_id: SocketId, url: &str) -> io::Result<EndpointId> {
            Err(util::other_io_error("FailingNetwork can only fail"))
        }
        fn bind(&self, socket_id: SocketId, url: &str) -> io::Result<EndpointId> {
            Err(util::other_io_error("FailingNetwork can only fail"))
        }
    }

    #[test]
    fn when_connect_fails() {
        let id = SocketId::from(1);
        let (tx, rx) = mpsc::channel();
        let proto = Box::new(TestProto) as Box<Protocol>;
        let network = FailingNetwork;
        let mut socket = Socket::new(id, tx, proto);

        socket.connect(&network, String::from("test://fake"));

        let reply = rx.recv().expect("Socket should have sent a reply to the connect request");

        match reply {
            Reply::Err(_) => {},
            _ => {
                assert!(false, "Socket should have replied an error to the connect request");
            },
        }
    }
}
