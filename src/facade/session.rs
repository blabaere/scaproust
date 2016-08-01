// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::thread;
use std::sync::mpsc;
use std::time;

use facade::socket::Socket;
use ctrl::{EventLoopSignal, EventLoop, run_event_loop};
use core::session::{Request, Reply};
use core::protocol::{Protocol, ProtocolCtor};
use util::*;

use mio;

pub struct Session {
    request_sender: mio::Sender<EventLoopSignal>,
    reply_receiver: mpsc::Receiver<Reply>
}

impl Session {
    pub fn new() -> io::Result<Session> {
        let mut builder = mio::EventLoopBuilder::new();

        builder.
            notify_capacity(4_096).
            messages_per_tick(256).
            timer_tick(time::Duration::from_millis(15)).
            timer_wheel_size(1_024).
            timer_capacity(4_096);

        let event_loop = try!(builder.build());
        let (tx, rx) = mpsc::channel();
        let session = Session { 
            request_sender: event_loop.channel(),
            reply_receiver: rx };

        thread::spawn(move || run_event_loop(event_loop, tx));

        Ok(session)
    }

    pub fn create_socket<T: Protocol + From<i32> + 'static>(&self) -> io::Result<Socket>
    {
        let protocol_ctor = Session::create_protocol_ctor::<T>();
        let request = Request::CreateSocket(protocol_ctor);

        try!(self.send_request(request));

        match self.reply_receiver.recv() {
            Ok(Reply::SocketCreated) => Ok(Socket{x: 5}),
            Ok(_)                            => Err(other_io_error("unexpected reply")),
            Err(_)                           => Err(other_io_error("channel closed"))
        }
    }

    fn create_protocol_ctor<T : Protocol + From<i32> + 'static>() -> ProtocolCtor {
        Box::new(move |value: i32| {
            Box::new(T::from(value)) as Box<Protocol>
        })
    }

    fn send_request(&self, request: Request) -> Result<(), io::Error> {
        let signal = EventLoopSignal::SessionRequest(request);

        self.request_sender.send(signal).map_err(from_notify_error)
    }
}
