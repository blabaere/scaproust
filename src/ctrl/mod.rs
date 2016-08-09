// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::thread;
use std::sync::mpsc;
use std::time;

use core::session;
use core::socket;

use mio;

#[doc(hidden)]
pub type EventLoop = mio::EventLoop<EventLoopHandler>;

/// Information flowing through the event loop so components can communicate with each others.
pub enum EventLoopSignal {
    SessionRequest(session::Request),
    SocketRequest(socket::SocketId, socket::Request)
    // SessionRequest
    // SessionReply
    // SocketRequest
    // SocketReply
    // PipeRequest
    // PipeReply
    // PipeEvent
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

struct EventLoopHandler {
    session: session::Session
}

impl mio::Handler for EventLoopHandler {
    type Timeout = ();
    type Message = EventLoopSignal;

    fn notify(&mut self, event_loop: &mut EventLoop, signal: Self::Message) {
        match signal {
            EventLoopSignal::SessionRequest(request) => self.session.process_request(request),
            _ => {}
        }
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
    }

    fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
    }

    fn interrupted(&mut self, _: &mut EventLoop) {
    }
}

