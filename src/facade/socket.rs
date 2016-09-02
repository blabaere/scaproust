// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;
use std::time::Duration;

use super::*;
use reactor;
use core::{SocketId, Message};
use core::socket::{Request, Reply};
use core::config::ConfigOption;
use core;
use io_error::*;

#[doc(hidden)]
pub type ReplyReceiver = mpsc::Receiver<Reply>;

#[doc(hidden)]
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
        self.req_tx.send(reactor::Request::Socket(self.socket_id, req)).map_err(from_send_error)
    }
}

/// Socket is what applications use to exchange messages.  
///   
/// It is an abstraction of an application's "connection" to a messaging topology.
/// Applications can have more than one Socket open at a time.
pub struct Socket {
    request_sender: RequestSender,
    reply_receiver: ReplyReceiver
}

impl Socket {
    #[doc(hidden)]
    pub fn new(request_tx: RequestSender, reply_rx: ReplyReceiver) -> Socket {
        Socket {
            request_sender: request_tx,
            reply_receiver: reply_rx
        }
    }

    #[doc(hidden)]
    pub fn id(&self) -> SocketId {
        self.request_sender.socket_id
    }

/*****************************************************************************/
/*                                                                           */
/* connect                                                                   */
/*                                                                           */
/*****************************************************************************/

    /// Adds a remote endpoint to the socket.
    /// The library would then try to connect to the specified remote endpoint.
    /// The addr argument consists of two parts as follows: `transport://address`.
    /// The transport specifies the underlying transport protocol to use.
    /// The meaning of the address part is specific to the underlying transport protocol.
    /// Note that bind and connect may be called multiple times on the same socket,
    /// thus allowing the socket to communicate with multiple heterogeneous endpoints.
    /// On success, returns an [Endpoint](struct.Endpoint.html) that can be later used to remove the endpoint from the socket.
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

    /// Adds a local endpoint to the socket. The endpoint can be then used by other applications to connect to.
    /// The addr argument consists of two parts as follows: `transport://address`.
    /// The transport specifies the underlying transport protocol to use.
    /// The meaning of the address part is specific to the underlying transport protocol.
    /// Note that bind and connect may be called multiple times on the same socket,
    /// thus allowing the socket to communicate with multiple heterogeneous endpoints.
    /// On success, returns an [Endpoint](struct.Endpoint.html) that can be later used to remove the endpoint from the socket.
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

    /// Sends a buffer.
    /// Which of the peers the buffer will be sent to is determined by the protocol.
    pub fn send(&mut self, buffer: Vec<u8>) -> io::Result<()> {
        self.send_msg(Message::from_body(buffer))
    }

    /// Sends a message.
    /// Which of the peers the message will be sent to is determined by the protocol.
    pub fn send_msg(&mut self, msg: Message) -> io::Result<()> {
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

    /// Receives a buffer.
    pub fn recv(&mut self) -> io::Result<Vec<u8>> {
        self.recv_msg().map(|msg| msg.into())
    }

    /// Receives a message.
    pub fn recv_msg(&mut self) -> io::Result<Message> {
        let request = Request::Recv;

        self.call(request, |reply| self.on_recv_reply(reply))
    }

    fn on_recv_reply(&self, reply: Reply) -> io::Result<Message> {
        match reply {
            Reply::Recv(msg) => Ok(msg),
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }

/*****************************************************************************/
/*                                                                           */
/* options                                                                   */
/*                                                                           */
/*****************************************************************************/

    /// Sets the timeout for send operation on the socket.  
    /// If message cannot be sent within the specified timeout, 
    /// an error with the kind `TimedOut` is returned. 
    /// None means infinite timeout. Default value is `None`.
    pub fn set_send_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_option(ConfigOption::SendTimeout(timeout))
    }

    /// Sets the timeout for recv operation on the socket.  
    /// If message cannot be received within the specified timeout, 
    /// an error with the kind `TimedOut` is returned. 
    /// `None` value means infinite timeout. Default value is `None`.
    pub fn set_recv_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_option(ConfigOption::RecvTimeout(timeout))
    }

    /// Sets outbound priority for endpoints subsequently added to the socket.  
    /// This option has no effect on socket types that send messages to all the peers.  
    /// However, if the socket type sends each message to a single peer (or a limited set of peers), 
    /// peers with high priority take precedence over peers with low priority. 
    /// Highest priority is 1, lowest priority is 16. Default value is 8.
    pub fn set_send_priority(&mut self, priority: u8) -> io::Result<()> {
        self.set_option(ConfigOption::SendPriority(priority))
    }

    /// Sets inbound priority for endpoints subsequently added to the socket.  
    /// This option has no effect on socket types that are not able to receive messages.  
    /// When receiving a message, messages from peer with higher priority 
    /// are received before messages from peer with lower priority. 
    /// Highest priority is 1, lowest priority is 16. Default value is 8.
    pub fn set_recv_priority(&mut self, priority: u8) -> io::Result<()> {
        self.set_option(ConfigOption::RecvPriority(priority))
    }

    /// This option, when set to `true`, disables Nagle’s algorithm.
    /// It also disables delaying of TCP acknowledgments. 
    /// Using this option improves latency at the expense of throughput.
    /// Default value is `false`.
    pub fn set_tcp_nodelay(&mut self, value: bool) -> io::Result<()> {
        self.set_option(ConfigOption::TcpNoDelay(value))
    }

    /// Sets a socket option.
    /// See [ConfigOption](core/config/enum.ConfigOption.html) to get the list of options.
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

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.send_request(Request::Close);
        let _ = self.recv_reply();
    }
}
