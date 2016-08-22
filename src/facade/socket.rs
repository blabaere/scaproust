// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;

use facade::*;
use core::socket::{SocketId, Request, Reply};
use core;
use ctrl::EventLoopSignal;
use io_error::*;

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
    fn child_sender(&self, eid: core::endpoint::EndpointId) -> endpoint::RequestSender {
        endpoint::RequestSender::new(self.signal_sender.clone(), self.socket_id, eid)
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

    pub fn connect(&mut self, url: &str) -> io::Result<endpoint::Endpoint> {
        let request = Request::Connect(From::from(url));

        self.call(request, |reply| self.on_connect_reply(reply))
    }

    fn on_connect_reply(&self, reply: Reply) -> io::Result<endpoint::Endpoint> {
        match reply {
            Reply::Connect(id) => {
                let request_tx = self.request_sender.child_sender(id);
                let ep = endpoint::Endpoint::new(request_tx);
                
                Ok(ep)
            },
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }

    fn call<T, F : FnOnce(Reply) -> io::Result<T>>(&self, request: Request, process: F) -> io::Result<T> {
        self.execute_request(request).and_then(process)
    }

    fn execute_request(&self, request: Request) -> io::Result<Reply> {
        self.send_request(request).and_then(|_| self.recv_reply())
    }

    fn send_request(&self, request: Request) -> io::Result<()> {
        self.request_sender.send(request)
    }

    fn recv_reply(&self) -> io::Result<Reply> {
        self.reply_receiver.receive()
    }

    fn unexpected_reply<T>(&self) -> io::Result<T> {
        Err(other_io_error("unexpected reply"))
    }
}