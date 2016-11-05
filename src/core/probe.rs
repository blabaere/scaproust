// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;
use std::io;

use super::SocketId;

pub enum Request {
    Poll,
    Close
}

pub enum Reply {
    Err(io::Error),
    Poll
}

pub trait Context {
    fn poll(&mut self, sid: SocketId);
}

pub struct Probe {
    reply_sender: Sender<Reply>,
}

impl Probe {
    pub fn new(reply_tx: Sender<Reply>) -> Probe {
        Probe {
            reply_sender: reply_tx
        }
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn poll(&mut self, _: &mut Context) {
        self.send_reply(Reply::Poll);
    }
}