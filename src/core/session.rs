// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use core::protocol::{Protocol, ProtocolCtor};

use std::sync::mpsc;

pub enum Request {
    CreateSocket(ProtocolCtor),
    CreateDevice,
    Shutdown
}

pub enum Reply {
    SocketCreated,
    SocketNotCreated,
    DeviceCreated,
    DeviceNotCreated,
    Shutdown
}

pub struct Session {
    reply_sender: mpsc::Sender<Reply>
}

impl Session {
    pub fn new(reply_tx: mpsc::Sender<Reply>) -> Session {
        Session {
            reply_sender: reply_tx
        }
    }
    pub fn process_request(&mut self, request: Request) {
        match request {
            Request::CreateSocket(ctor) => self.add_socket(ctor),
            _ => {}
        }
    }
    fn add_socket(&mut self, protocol_ctor: ProtocolCtor) {
        let protocol = protocol_ctor.call_box((5,));
        self.reply_sender.send(Reply::SocketCreated);
    }
}