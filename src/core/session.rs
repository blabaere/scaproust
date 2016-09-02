// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc;
use std::io;

use core::{SocketId, socket};
use sequence::Sequence;

pub enum Request {
    CreateSocket(socket::ProtocolCtor),
    CreateDevice,
    Shutdown
}

pub enum Reply {
    Err(io::Error),
    SocketCreated(SocketId, mpsc::Receiver<socket::Reply>),
    DeviceCreated,
    Shutdown
}

pub struct Session {
    reply_sender: mpsc::Sender<Reply>,
    sockets: SocketCollection
}

struct SocketCollection {
    ids: Sequence,
    sockets: HashMap<SocketId, socket::Socket>
}

impl Session {
    pub fn new(seq: Sequence, reply_tx: mpsc::Sender<Reply>) -> Session {
        Session {
            reply_sender: reply_tx,
            sockets: SocketCollection::new(seq)
        }
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn add_socket(&mut self, protocol_ctor: socket::ProtocolCtor) {
        let (tx, rx) = mpsc::channel();
        let protocol_ctor_args = (tx.clone(),);
        let protocol = protocol_ctor.call_box(protocol_ctor_args);
        let id = self.sockets.add(tx, protocol);

        self.send_reply(Reply::SocketCreated(id, rx));
    }

    pub fn get_socket_mut<'a>(&'a mut self, id: SocketId) -> Option<&'a mut socket::Socket> {
        self.sockets.get_socket_mut(id)
    }

    pub fn remove_socket(&mut self, sid: SocketId) {
        self.sockets.remove(sid);
    }
}

impl SocketCollection {
    fn new(seq: Sequence) -> SocketCollection {
        SocketCollection {
            ids: seq,
            sockets: HashMap::new()
        }
    }

    fn add(&mut self, reply_tx: mpsc::Sender<socket::Reply>, proto: Box<socket::Protocol>) -> SocketId {
        let id = SocketId::from(self.ids.next());
        let socket = socket::Socket::new(id, reply_tx, proto);

        self.sockets.insert(id, socket);

        id
    }

    fn get_socket_mut<'a>(&'a mut self, id: SocketId) -> Option<&'a mut socket::Socket> {
        self.sockets.get_mut(&id)
    }

    fn remove(&mut self, id: SocketId) {
        self.sockets.remove(&id);
    }
}
