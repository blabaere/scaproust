// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;
use std::time::Duration;

use super::*;
use reactor;
use core::{ProbeId, PollRes};
use core::probe::{Request, Reply};
use io_error::*;

#[doc(hidden)]
pub type ReplyReceiver = mpsc::Receiver<Reply>;

#[doc(hidden)]
pub struct RequestSender {
    req_tx: EventLoopRequestSender,
    probe_id: ProbeId
}

/// Probe is what applications use to poll sockets.  
pub struct Probe {
    request_sender: RequestSender,
    reply_receiver: ReplyReceiver
}

impl RequestSender {
    pub fn new(tx: EventLoopRequestSender, id: ProbeId) -> RequestSender {
        RequestSender {
            req_tx: tx,
            probe_id: id
        }
    }
    fn send(&self, req: Request) -> io::Result<()> {
        self.req_tx.send(reactor::Request::Probe(self.probe_id, req)).map_err(from_send_error)
    }
}

impl Probe {
    #[doc(hidden)]
    pub fn new(
        request_tx: RequestSender, 
        reply_rx: ReplyReceiver) -> Probe {

        Probe {
            request_sender: request_tx,
            reply_receiver: reply_rx
        }
    }

    /// Checks the sockets and reports whether it’s possible to send a message to the socket and/or receive a message from each socket.
    /// Returns a vector of [PollRes](struct.PollRes.html), one for each [PollReq](struct.PollReq.html) provided a build time.
    pub fn poll(&mut self, timeout: Duration) -> io::Result<Vec<PollRes>> {
        let request = Request::Poll(timeout);

        self.call(request, |reply| self.on_poll_reply(reply))
    }

    fn on_poll_reply(&self, reply: Reply) -> io::Result<Vec<PollRes>> {
        match reply {
            Reply::Poll(x)   => Ok(x),
            Reply::Err(e) => Err(e),
            Reply::Closed => Err(other_io_error("unexpected reply"))
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
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.send_request(Request::Close);
        let _ = self.recv_reply();
    }
}
