// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::ops::Deref;
use std::rc::Rc;
use std::sync::mpsc;
use std::io;

use facade::*;
use facade::endpoint::Endpoint;
use core::socket::{SocketId, Request, Reply};
use ctrl::EventLoopSignal;
use util::*;

use mio;

pub type ReplyReceiver = mpsc::Receiver<Reply>;

pub struct RequestSender {
    signal_sender: SignalSender,
    socket_id: SocketId
}

impl RequestSender {
    pub fn new(signal_tx: SignalSender, id: SocketId) -> RequestSender {
        RequestSender {
            signal_sender: signal_tx,
            socket_id: id
        }
    }
}

impl Sender<Request> for RequestSender {
    fn send(&self, req: Request) -> io::Result<()> {
        self.signal_sender.send(EventLoopSignal::SocketRequest(self.socket_id, req))
    }
}

pub struct Socket {
    request_sender: RequestSender,
    reply_receiver: ReplyReceiver
}

impl Socket {
    pub fn new(request_tx: RequestSender, reply_rx: ReplyReceiver) -> Socket {
        Socket {
            request_sender: request_tx,
            reply_receiver: reply_rx
        }
    }

    pub fn connect(&mut self, url: &str) -> io::Result<Endpoint> {
        unreachable!()
    }
}