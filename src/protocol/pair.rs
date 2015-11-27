// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use super::Protocol;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::{ SocketNotify, SocketOption, SocketEvtSignal };
use EventLoop;
use EventLoopAction;
use Message;

pub struct Pair {
    endpoint: Option<Endpoint>
}

impl Pair {
    pub fn new(evt_tx: Rc<Sender<SocketNotify>>) -> Pair {
        Pair { 
            endpoint: None
        }
    }

    fn on_pipe_connected(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            if endpoint.token() == tok {
                endpoint.on_pipe_connected(event_loop);
            }
        }
    }

    fn on_pipe_ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            if endpoint.token() == tok {
                endpoint.on_pipe_ready(event_loop, events);
            }
        }
    }
}

impl Protocol for Pair {

    fn id(&self) -> u16 {
        SocketType::Pair.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Pair.id()
    }

    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
        self.endpoint = Some(Endpoint::new(token, pipe));
    }

    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
        if Some(token) == self.endpoint.as_ref().map(|e| e.token()) {
            self.endpoint.take().map(|e| e.remove())
        } else {
            None
        }
    }

    fn handle_evt(&mut self, event_loop: &mut EventLoop, signal: SocketEvtSignal) {
        debug!("handle_signal");
        match signal {
            SocketEvtSignal::Connected(tok) => self.on_pipe_connected(event_loop, tok)
        }
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
    }

    fn recv(&mut self, event_loop: &mut EventLoop, _: EventLoopAction) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.recv(event_loop);
        }
    }
    
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        debug!("[{:?}] ready", tok);
        self.on_pipe_ready(event_loop, tok, events);
    }

    fn set_option(&mut self, _: &mut EventLoop, _: SocketOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {}
    fn on_request_timeout(&mut self, _: &mut EventLoop) {}
}