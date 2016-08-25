// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;
use std::fmt;

use sequence::Sequence;
use super::message::Message;
use super::protocol::Protocol;
use super::endpoint::{EndpointId, Pipe, Acceptor};
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
    pipes: HashMap<EndpointId, Pipe>,
    acceptors: HashMap<EndpointId, Acceptor>,
}

impl Socket {
    pub fn new(id: SocketId, reply_tx: Sender<Reply>, proto: Box<Protocol>) -> Socket {
        Socket {
            id: id,
            reply_sender: reply_tx,
            protocol: proto,
            pipes: HashMap::new(),
            acceptors: HashMap::new()
        }
    }

    fn get_protocol_ids(&self) -> (u16, u16) {
        let proto_id = self.protocol.id();
        let peer_proto_id = self.protocol.peer_id();

        (proto_id, peer_proto_id)
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn connect(&mut self, network: &mut Network, url: String) {
        let pids = self.get_protocol_ids();

        match network.connect(self.id, &url, pids) {
            Ok(id) => self.on_connect_success(network, url, id),
            Err(e) => self.on_connect_error(e)
        };
    }

    fn on_connect_success(&mut self, network: &mut Network, url: String, eid: EndpointId) {
        let pipe = Pipe::new_created(eid, url);

        pipe.open(network);

        self.pipes.insert(eid, pipe);
        self.send_reply(Reply::Connect(eid));
    }

    fn on_connect_error(&mut self, err: io::Error) {
        self.send_reply(Reply::Err(err));
    }

    pub fn bind(&mut self, network: &mut Network, url: String) {
        let pids = self.get_protocol_ids();

        match network.bind(self.id, &url, pids) {
            Ok(id) => self.on_bind_success(network, url, id),
            Err(e) => self.on_bind_error(e)
        };
    }

    fn on_bind_success(&mut self, network: &mut Network, url: String, eid: EndpointId) {
        let acceptor = Acceptor::new(eid, url);

        acceptor.open(network);

        self.acceptors.insert(eid, acceptor);
        self.send_reply(Reply::Bind(eid));
    }

    fn on_bind_error(&mut self, err: io::Error) {
        self.send_reply(Reply::Err(err));
    }

    pub fn send(&mut self, network: &mut Network, msg: Message) {
        self.protocol.send(network, msg);
    }

    pub fn on_send_ack(&mut self, network: &mut Network, eid: EndpointId) {
        self.protocol.on_send_ack(network, eid);
    }

    pub fn recv(&mut self, network: &mut Network) {
        self.protocol.recv(network);
    }

    pub fn on_pipe_opened(&mut self, network: &mut Network, eid: EndpointId) {
        if let Some(pipe) = self.pipes.remove(&eid) {
            self.protocol.add_pipe(network, eid, pipe);
        }
    }

}

pub struct SocketCollection {
    ids: Sequence,
    sockets: HashMap<SocketId, Socket>
}

impl SocketCollection {
    pub fn new(seq: Sequence) -> SocketCollection {
        SocketCollection {
            ids: seq,
            sockets: HashMap::new()
        }
    }

    pub fn add(&mut self, reply_tx: Sender<Reply>, proto: Box<Protocol>) -> SocketId {
        let id = SocketId::from(self.ids.next());
        let socket = Socket::new(id, reply_tx, proto);

        self.sockets.insert(id, socket);

        id
    }

    pub fn get_socket_mut<'a>(&'a mut self, id: SocketId) -> Option<&'a mut Socket> {
        self.sockets.get_mut(&id)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::io;

    use super::*;
    use core::protocol::Protocol;
    use core::network::Network;
    use core::endpoint::{EndpointId, Pipe};
    use io_error::*;
    use Message;

    struct TestProto;

    impl Protocol for TestProto {
        fn id(&self) -> u16 {0}
        fn peer_id(&self) -> u16 {0}
        fn add_pipe(&mut self, network: &mut Network, eid: EndpointId, pipe: Pipe) {}
        fn remove_pipe(&mut self, network: &mut Network, eid: EndpointId) -> Option<Pipe> {None}
        fn send(&mut self, network: &mut Network, msg: Message) {}
        fn on_send_ack(&mut self, network: &mut Network, eid: EndpointId) {}
        fn recv(&mut self, network: &mut Network) {}
    }

    struct FailingNetwork;

    impl Network for FailingNetwork {
        fn connect(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn bind(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn open(&mut self, endpoint_id: EndpointId) {
        }
        fn close(&mut self, endpoint_id: EndpointId) {
        }
        fn send(&mut self, endpoint_id: EndpointId, msg: Rc<Message>) {
        }
        fn recv(&mut self, endpoint_id: EndpointId) {
        }
    }

    #[test]
    fn when_connect_fails() {
        let id = SocketId::from(1);
        let (tx, rx) = mpsc::channel();
        let proto = Box::new(TestProto) as Box<Protocol>;
        let mut network = FailingNetwork;
        let mut socket = Socket::new(id, tx, proto);

        socket.connect(&mut network, String::from("test://fake"));

        let reply = rx.recv().expect("Socket should have sent a reply to the connect request");

        match reply {
            Reply::Err(_) => {},
            _ => {
                assert!(false, "Socket should have replied an error to the connect request");
            },
        }
    }

    struct WorkingNetwork(EndpointId);

    impl Network for WorkingNetwork {
        fn connect(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId> {
            Ok(self.0)
        }
        fn bind(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId> {
            Ok(self.0)
        }
        fn open(&mut self, endpoint_id: EndpointId) {}
        fn close(&mut self, endpoint_id: EndpointId) {}
        fn send(&mut self, endpoint_id: EndpointId, msg: Rc<Message>) {}
        fn recv(&mut self, endpoint_id: EndpointId) {}
    }

    #[test]
    fn when_connect_succeeds() {
        let id = SocketId::from(1);
        let (tx, rx) = mpsc::channel();
        let proto = Box::new(TestProto) as Box<Protocol>;
        let mut network = WorkingNetwork(EndpointId::from(1));
        let mut socket = Socket::new(id, tx, proto);

        socket.connect(&mut network, String::from("test://fake"));

        let reply = rx.recv().expect("Socket should have sent a reply to the connect request");

        match reply {
            Reply::Connect(eid) => {
                assert_eq!(EndpointId::from(1), eid);
            },
            _ => {
                assert!(false, "Socket should have replied an ack to the connect request");
            },
        }
    }
}
