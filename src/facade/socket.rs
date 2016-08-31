// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;
use std::time::Duration;

use super::*;
use ctrl;
use ctrl::reactor;
use core::{SocketId, Message};
use core::socket::{Request, Reply};
use core::config::ConfigOption;
use core;
use io_error::*;

pub type ReplyReceiver = mpsc::Receiver<Reply>;

pub struct RequestSender {
    req_tx: EventLoopRequestSender,
    socket_id: SocketId
}

impl RequestSender {
    pub fn new(tx: EventLoopRequestSender, id: SocketId) -> RequestSender {
        RequestSender {
            req_tx: tx,
            socket_id: id
        }
    }
    fn child_sender(&self, eid: core::EndpointId) -> endpoint::RequestSender {
        endpoint::RequestSender::new(self.req_tx.clone(), self.socket_id, eid)
    }
    fn send(&self, req: Request) -> io::Result<()> {
        self.req_tx.send(ctrl::Request::Socket(self.socket_id, req)).map_err(from_notify_error)
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

/*****************************************************************************/
/*                                                                           */
/* connect                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn connect(&mut self, url: &str) -> io::Result<endpoint::Endpoint> {
        let request = Request::Connect(From::from(url));

        self.call(request, |reply| self.on_connect_reply(reply))
    }

    fn on_connect_reply(&self, reply: Reply) -> io::Result<endpoint::Endpoint> {
        match reply {
            Reply::Connect(id) => {
                let request_tx = self.request_sender.child_sender(id);
                let ep = endpoint::Endpoint::new(request_tx, true);
                
                Ok(ep)
            },
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }

/*****************************************************************************/
/*                                                                           */
/* bind                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn bind(&mut self, url: &str) -> io::Result<endpoint::Endpoint> {
        let request = Request::Bind(From::from(url));

        self.call(request, |reply| self.on_bind_reply(reply))
    }

    fn on_bind_reply(&self, reply: Reply) -> io::Result<endpoint::Endpoint> {
        match reply {
            Reply::Bind(id) => {
                let request_tx = self.request_sender.child_sender(id);
                let ep = endpoint::Endpoint::new(request_tx, false);
                
                Ok(ep)
            },
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }

/*****************************************************************************/
/*                                                                           */
/* send                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn send(&mut self, buffer: Vec<u8>) -> io::Result<()> {
        let msg = Message::from_body(buffer);
        let request = Request::Send(msg);

        self.call(request, |reply| self.on_send_reply(reply))
    }

    fn on_send_reply(&self, reply: Reply) -> io::Result<()> {
        match reply {
            Reply::Send => Ok(()),
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn recv(&mut self) -> io::Result<Vec<u8>> {
        let request = Request::Recv;

        self.call(request, |reply| self.on_recv_reply(reply))
    }

    fn on_recv_reply(&self, reply: Reply) -> io::Result<Vec<u8>> {
        match reply {
            Reply::Recv(msg) => Ok(msg.into()),
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }

/*****************************************************************************/
/*                                                                           */
/* options                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn set_send_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_option(ConfigOption::SendTimeout(timeout))
    }

    pub fn set_recv_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_option(ConfigOption::RecvTimeout(timeout))
    }

    pub fn set_option(&mut self, cfg_opt: ConfigOption) -> io::Result<()> {
        let request = Request::SetOption(cfg_opt);

        self.call(request, |reply| self.on_set_option_reply(reply))
    }

    fn on_set_option_reply(&self, reply: Reply) -> io::Result<()> {
        match reply {
            Reply::SetOption => Ok(()),
            Reply::Err(e)    => Err(e),
            _ => self.unexpected_reply()
        }
    }

/*****************************************************************************/
/*                                                                           */
/* backend                                                                   */
/*                                                                           */
/*****************************************************************************/

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