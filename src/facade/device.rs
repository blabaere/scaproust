// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
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

/// A device to forward messages between sockets, working like a message broker.
/// It can be used to build complex network topologies.
pub trait Device : Send {
    /// This function loops until it hits an error.
    /// To break the loop and make the `run` function exit, 
    /// drop the session that created the device.
    fn run(self: Box<Self>) -> io::Result<()>;
}

/*****************************************************************************/
/*                                                                           */
/* RELAY DEVICE                                                              */
/*                                                                           */
/*****************************************************************************/

#[doc(hidden)]
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
            socket.recv_msg().and_then(|msg| socket.send_msg(msg))?;
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* BRIDGE DEVICE                                                             */
/*                                                                           */
/*****************************************************************************/

#[doc(hidden)]
pub type ReplyReceiver = mpsc::Receiver<Reply>;

#[doc(hidden)]
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

#[doc(hidden)]
pub struct Bridge {
    request_sender: RequestSender,
    reply_receiver: ReplyReceiver,
    left: Option<socket::Socket>,
    right: Option<socket::Socket>
}

impl Bridge {
    #[doc(hidden)]
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

    fn run_once(&mut self, left: &mut socket::Socket, right: &mut socket::Socket) -> io::Result<()> {
        if let Reply::Check(l, r) = self.execute_request(Request::Check)? {
            match (l, r) {
                (true, true) => exchange_msg(left, right),
                (true, _)    => forward_msg(left, right),
                (_, true)    => forward_msg(right, left),
                (_, _)       => Ok(())
            }
        } else {
            Err(other_io_error("unexpected reply"))
        }

    }
}

impl Device for Bridge {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        let mut left = self.left.take().unwrap();
        let mut right = self.right.take().unwrap();

        loop {
            self.run_once(&mut left, &mut right)?;
        }
    }
}

fn forward_msg(from: &mut socket::Socket, to: &mut socket::Socket) -> io::Result<()> {
    from.recv_msg().and_then(|msg| to.send_msg(msg))
}

fn exchange_msg(left: &mut socket::Socket, right: &mut socket::Socket) -> io::Result<()> {
    let from_left = left.recv_msg()?;
    let from_right = right.recv_msg()?;

    right.send_msg(from_left).and_then(|_| left.send_msg(from_right))
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.send_request(Request::Close);
        let _ = self.recv_reply();
    }
}
