// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;
use std::thread;
use std::sync::mpsc;
use std::time;

use mio;

use facade::*;
use facade::socket::Socket;
use ctrl::{EventLoopSignal, run_event_loop};
use core::session::{Request, Reply};
use core::protocol::{Protocol, ProtocolCtor};
use util::*;

type SignalSender = Rc<mio::Sender<EventLoopSignal>>;
type ReplyReceiver = mpsc::Receiver<Reply>;

pub struct SessionBuilder;

impl SessionBuilder {

    pub fn build() -> io::Result<Session> {

        let mut builder = mio::EventLoopBuilder::new();

        builder.
            notify_capacity(4_096).
            messages_per_tick(256).
            timer_tick(time::Duration::from_millis(15)).
            timer_wheel_size(1_024).
            timer_capacity(4_096);

        let event_loop = try!(builder.build());
        let (tx, rx) = mpsc::channel();
        let request_tx = Rc::new(event_loop.channel());
        let reply_rx = rx;
        let session = Session::new(request_tx, reply_rx);

        thread::spawn(move || run_event_loop(event_loop, tx));

        Ok(session)
    }}

pub struct Session {
    request_sender: SignalSender,
    reply_receiver: ReplyReceiver
}

impl Session {

    fn new(request_tx: SignalSender, reply_rx: ReplyReceiver) -> Session {
        Session {
            request_sender: request_tx,
            reply_receiver: reply_rx
        }
    }

    pub fn create_socket<T: Protocol + From<i32> + 'static>(&self) -> io::Result<Socket>
    {
        let protocol_ctor = Session::create_protocol_ctor::<T>();
        let request = Request::CreateSocket(protocol_ctor);

        self.call(request, |reply| self.on_create_socket_reply(reply))
    }

    fn create_protocol_ctor<T : Protocol + From<i32> + 'static>() -> ProtocolCtor {
        Box::new(move |value: i32| {
            Box::new(T::from(value)) as Box<Protocol>
        })
    }

    fn on_create_socket_reply(&self, reply: Reply) -> io::Result<Socket> {
        if let Reply::SocketCreated(rx) = reply {
            Ok(Socket::new(self.request_sender.clone(), rx))
        } else {
            self.unexpected_reply()
        }
    }

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
        match self.reply_receiver.recv() {
            Ok(reply) => Ok(reply),
            Err(_)    => Err(other_io_error("channel closed"))
        }
    }
}
