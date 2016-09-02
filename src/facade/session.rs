// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::thread;
use std::sync::mpsc;

use mio;

use super::*;
use reactor;
use reactor::dispatcher;
use core::session::{Request, Reply};
use core::socket::{Protocol, ProtocolCtor};
use core;
use io_error::*;

#[doc(hidden)]
type ReplyReceiver = mpsc::Receiver<Reply>;

#[doc(hidden)]
struct RequestSender {
    req_tx: EventLoopRequestSender
}

impl RequestSender {
    fn new(tx: EventLoopRequestSender) -> RequestSender {
        RequestSender { req_tx: tx }
    }
    fn socket_sender(&self, socket_id: core::SocketId) -> socket::RequestSender {
        socket::RequestSender::new(self.req_tx.clone(), socket_id)
    }
    fn device_sender(&self, device_id: core::DeviceId) -> device::RequestSender {
        device::RequestSender::new(self.req_tx.clone(), device_id)
    }
    fn send(&self, req: Request) -> io::Result<()> {
        self.req_tx.send(reactor::Request::Session(req)).map_err(from_send_error)
    }
}

/// Creates the session and starts the I/O thread.
#[derive(Default)]
pub struct SessionBuilder;

impl SessionBuilder {

    pub fn new() -> SessionBuilder {
        SessionBuilder
    }

    pub fn build() -> io::Result<Session> {

        let (reply_tx, reply_rx) = mpsc::channel();
        let (request_tx, request_rx) = mio::channel::channel();
        let session = Session::new(RequestSender::new(request_tx), reply_rx);

        thread::spawn(move || dispatcher::Dispatcher::dispatch(request_rx, reply_tx));

        Ok(session)
    }}

/// Creates sockets and devices.
pub struct Session {
    request_sender: RequestSender,
    reply_receiver: ReplyReceiver
}

impl Session {

    fn new(request_tx: RequestSender, reply_rx: ReplyReceiver) -> Session {
        Session {
            request_sender: request_tx,
            reply_receiver: reply_rx
        }
    }

/*****************************************************************************/
/*                                                                           */
/* Create socket                                                             */
/*                                                                           */
/*****************************************************************************/

    /// Creates a socket with the specified protocol, which in turn determines its exact semantics.
    /// See [the proto module](proto/index.html) for a list of built-in protocols.
    /// The newly created socket is initially not associated with any endpoints.
    /// In order to establish a message flow at least one endpoint has to be added to the socket 
    /// using [connect](struct.Socket.html#method.connect) and [bind](struct.Socket.html#method.bind) methods.
    pub fn create_socket<T>(&mut self) -> io::Result<socket::Socket>
    where T : Protocol + From<mpsc::Sender<core::socket::Reply>> + 'static
    {
        let protocol_ctor = Session::create_protocol_ctor::<T>();
        let request = Request::CreateSocket(protocol_ctor);

        self.call(request, |reply| self.on_create_socket_reply(reply))
    }

    fn create_protocol_ctor<T>() -> ProtocolCtor 
    where T : Protocol + From<mpsc::Sender<core::socket::Reply>> + 'static
    {
        Box::new(move |sender: mpsc::Sender<core::socket::Reply>| {
            Box::new(T::from(sender)) as Box<Protocol>
        })
    }

    fn on_create_socket_reply(&self, reply: Reply) -> io::Result<socket::Socket> {
        match reply {
            Reply::SocketCreated(id, rx) => {
                let sender = self.request_sender.socket_sender(id);
                let sock = socket::Socket::new(sender, rx);
                
                Ok(sock)
            },
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }

/*****************************************************************************/
/*                                                                           */
/* Create device                                                             */
/*                                                                           */
/*****************************************************************************/

    /// Creates a loopback device that loops and sends any messages received from the socket back to itself.
    pub fn create_relay_device(&self, socket: socket::Socket) -> io::Result<Box<device::Device>> {
        Ok(box device::Relay::new(socket))
    }

    /// Creates a bridge device to forward messages between two sockets. 
    /// It loops and sends any messages received from `left` to `right` and vice versa.
    pub fn create_bridge_device(&mut self, left: socket::Socket, right: socket::Socket) -> io::Result<Box<device::Device>> {
        let request = Request::CreateDevice(left.id(), right.id());

        self.call(request, |reply| self.on_create_device_reply(reply, left, right))
    }

    fn on_create_device_reply(&self, reply: Reply, left: socket::Socket, right: socket::Socket) -> io::Result<Box<device::Device>> {
        match reply {
            Reply::DeviceCreated(id, rx) => {
                let sender = self.request_sender.device_sender(id);
                let bridge = device::Bridge::new(sender, rx, left, right);
                
                Ok(box bridge)
            },
            Reply::Err(e) => Err(e),
            _ => self.unexpected_reply()
        }
    }


/*****************************************************************************/
/*                                                                           */
/* backend                                                                   */
/*                                                                           */
/*****************************************************************************/

    fn unexpected_reply<T>(&self) -> io::Result<T> {
        Err(other_io_error("unexpected reply"))
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

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.send_request(Request::Shutdown);
        let _ = self.recv_reply();
    }
}
