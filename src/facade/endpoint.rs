// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use facade::*;
use core::socket::SocketId;
use core::endpoint::{EndpointId, Request};
use ctrl::EventLoopSignal;

pub struct RequestSender {
    signal_sender: SignalSender,
    socket_id: SocketId,
    id: EndpointId
}

impl RequestSender {
    pub fn new(signal_tx: SignalSender, socket_id: SocketId, id: EndpointId) -> RequestSender {
        RequestSender {
            signal_sender: signal_tx,
            socket_id: socket_id,
            id: id
        }
    }
}

impl Sender<Request> for RequestSender {
    fn send(&self, req: Request) -> io::Result<()> {
        self.signal_sender.send(EventLoopSignal::EndpointRequest(self.socket_id, self.id, req))
    }
}

pub struct Endpoint {
    request_sender: RequestSender
}

impl Endpoint {
    pub fn new(request_tx: RequestSender) -> Endpoint {
        Endpoint {
            request_sender: request_tx
        }
    }

    pub fn close(self) -> io::Result<()> {
        self.request_sender.send(Request::Close)
    }
}