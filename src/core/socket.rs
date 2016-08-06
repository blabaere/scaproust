// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;
use std::io;

use message::Message;
use core::protocol::Protocol;

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
