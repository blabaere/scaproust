// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::thread;
use std::sync::mpsc;
use std::time;

use mio;

use core::session;
use core::socket;
use core::endpoint;
use core::network;

#[doc(hidden)]
pub type EventLoop = mio::EventLoop<EventLoopHandler>;

/// Information flowing through the event loop so components can communicate with each others.
pub enum EventLoopSignal {
    SessionRequest(session::Request),
    SocketRequest(socket::SocketId, socket::Request),
    EndpointRequest(socket::SocketId, endpoint::EndpointId, endpoint::Request)
    // PipeCmd : Send, Recv, Close
    // PipeEvent : Opened, Closed, Sent, Received(Message), Error
    // AcceptorCmd : Close
    // AcceptorEvent : Closed, Error, Accepted(Vec<Box<Pipe>>)
}

pub fn run_event_loop(mut event_loop: EventLoop, reply_tx: mpsc::Sender<session::Reply>) {
    let mut handler = EventLoopHandler { 
        session: session::Session::new(reply_tx) 
    };
    let exec = event_loop.run(&mut handler);

    /*match exec {
        Ok(_) => debug!("event loop exited"),
        Err(e) => error!("event loop failed to run: {}", e)
    }*/
}

pub struct EventLoopHandler {
    session: session::Session
}

impl EventLoopHandler {
    fn process_session_request(&mut self, req: session::Request) {
        match req {
            session::Request::CreateSocket(ctor) => self.session.add_socket(ctor),
            _ => {}
        }
    }
    fn process_socket_request(&mut self, id: socket::SocketId, req: socket::Request) {
        let mut network = TestNetwork(666);

        match req {
            socket::Request::Connect(url) => self.session.do_on_socket_mut(id, |socket| socket.connect(&mut network, url)),
            _ => {}
        }

        
    }
}

impl mio::Handler for EventLoopHandler {
    type Timeout = ();
    type Message = EventLoopSignal;

    fn notify(&mut self, event_loop: &mut EventLoop, signal: Self::Message) {
        match signal {
            EventLoopSignal::SessionRequest(request) => self.process_session_request(request),
            EventLoopSignal::SocketRequest(id, request) => self.process_socket_request(id, request),
            _ => {}
        }
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        // here find the registered io readiness observer
        // then pass and call the readiness notification method
    }

    fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
    }

    fn interrupted(&mut self, _: &mut EventLoop) {
    }
}

#[derive(Debug)]
struct TestNetwork(usize);

impl network::Network for TestNetwork {
    fn connect(&mut self, socket_id: socket::SocketId, url: &str) -> io::Result<endpoint::EndpointId> {
        Ok(From::from(self.0))
    }
    fn bind(&mut self, socket_id: socket::SocketId, url: &str) -> io::Result<endpoint::EndpointId> {
        Ok(From::from(self.0))
    }
}

