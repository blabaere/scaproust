// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;

use core::protocol::{Protocol, ProtocolCtor};
use core::socket;
use sequence::Sequence;

pub enum Request {
    CreateSocket(ProtocolCtor),
    CreateDevice,
    Shutdown
}

pub enum Reply {
    Err(io::Error),
    SocketCreated(socket::SocketId, mpsc::Receiver<socket::Reply>),
    DeviceCreated,
    Shutdown
}

pub struct Session {
    reply_sender: mpsc::Sender<Reply>,
    sockets: socket::SocketCollection
}

impl Session {
    pub fn new(seq: Sequence, reply_tx: mpsc::Sender<Reply>) -> Session {
        Session {
            reply_sender: reply_tx,
            sockets: socket::SocketCollection::new(seq)
        }
    }

    pub fn add_socket(&mut self, protocol_ctor: ProtocolCtor) {
        let (tx, rx) = mpsc::channel();
        let protocol_ctor_args = (tx.clone(),);
        let protocol = protocol_ctor.call_box(protocol_ctor_args);
        let id = self.sockets.add(tx, protocol);

        self.reply_sender.send(Reply::SocketCreated(id, rx));
    }

    pub fn do_on_socket<F>(&self, id: socket::SocketId, f: F) where F : FnOnce(&socket::Socket) {
        self.sockets.do_on_socket(id, f)
    }

    pub fn do_on_socket_mut<F>(&mut self, id: socket::SocketId, f: F) where F : FnOnce(&mut socket::Socket) {
        self.sockets.do_on_socket_mut(id, f)
    }

    pub fn get_socket_mut<'a>(&'a mut self, id: socket::SocketId) -> Option<&'a mut socket::Socket> {
        self.sockets.get_socket_mut(id)
    }

}
