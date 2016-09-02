// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.


use std::sync::mpsc;
use std::io;

use super::*;
use reactor;
use core::DeviceId;
use core::device::{Request, Reply};
use io_error::*;

pub trait Device : Send {
    fn run(self: Box<Self>) -> io::Result<()>;
}

/*****************************************************************************/
/*                                                                           */
/* RELAY DEVICE                                                              */
/*                                                                           */
/*****************************************************************************/

pub struct Relay {
    socket: Option<socket::Socket>
}

impl Relay {
    pub fn new(s: socket::Socket) -> Relay {
        Relay { socket: Some(s) }
    }
}

impl Device for Relay {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        let mut socket = self.socket.take().unwrap();
        loop {
            try!(socket.recv_msg().and_then(|msg| socket.send_msg(msg)));
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* BRIDGE DEVICE                                                             */
/*                                                                           */
/*****************************************************************************/

pub type ReplyReceiver = mpsc::Receiver<Reply>;

pub struct RequestSender {
    req_tx: EventLoopRequestSender,
    device_id: DeviceId
}

impl RequestSender {
    pub fn new(tx: EventLoopRequestSender, id: DeviceId) -> RequestSender {
        RequestSender {
            req_tx: tx,
            device_id: id
        }
    }
    fn send(&self, req: Request) -> io::Result<()> {
        self.req_tx.send(reactor::Request::Device(self.device_id, req)).map_err(from_send_error)
    }
}

pub struct Bridge {
    request_sender: RequestSender,
    reply_receiver: ReplyReceiver,
    left: Option<socket::Socket>,
    right: Option<socket::Socket>
}

impl Bridge {
    pub fn new(
        request_tx: RequestSender, 
        reply_rx: ReplyReceiver,
        left: socket::Socket,
        right: socket::Socket) -> Bridge {

        Bridge {
            request_sender: request_tx,
            reply_receiver: reply_rx,
            left: Some(left),
            right: Some(right)
        }
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

impl Device for Bridge {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        let mut left = self.left.take().unwrap();
        let mut right = self.right.take().unwrap();

        loop {
            let reply = try!(self.execute_request(Request::Check));

            match reply {
                Reply::Check(l, r) => {
                    if l {
                        try!(forward_msg(&mut left, &mut right));
                    }
                    if r {
                        try!(forward_msg(&mut right, &mut left));
                    }
                }
            }
        }
    }
}

fn forward_msg(from: &mut socket::Socket, to: &mut socket::Socket) -> io::Result<()> {
    from.recv_msg().and_then(|msg| to.send_msg(msg))
}
