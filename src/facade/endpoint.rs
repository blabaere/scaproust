// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use super::*;
use reactor;
use core::{SocketId, EndpointId};
use core::endpoint::Request;
use io_error::*;

pub struct RequestSender {
    req_tx: EventLoopRequestSender,
    socket_id: SocketId,
    id: EndpointId
}

impl RequestSender {
    pub fn new(tx: EventLoopRequestSender, sid: SocketId, eid: EndpointId) -> RequestSender {
        RequestSender {
            req_tx: tx,
            socket_id: sid,
            id: eid,
        }
    }
    fn send(&self, req: Request) -> io::Result<()> {
        self.req_tx.send(reactor::Request::Endpoint(self.socket_id, self.id, req)).map_err(from_send_error)
    }
}

pub struct Endpoint {
    request_sender: RequestSender,
    remote: bool
}

impl Endpoint {
    pub fn new(request_tx: RequestSender, remote: bool) -> Endpoint {
        Endpoint {
            request_sender: request_tx,
            remote: remote
        }
    }

    pub fn close(self) -> io::Result<()> {
        self.request_sender.send(Request::Close(self.remote))
    }
}