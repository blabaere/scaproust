// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use pipe::*;
use EventLoop;
use Message;

pub struct Endpoint {
    token: mio::Token,
    pipe: Pipe
}

impl Endpoint {
    pub fn new(token: mio::Token, pipe: Pipe) -> Endpoint {
        Endpoint { 
            token: token,
            pipe: pipe
        }
    }

    pub fn token(&self) -> mio::Token {
        self.token
    }

    pub fn on_pipe_connected(&mut self, event_loop: &mut EventLoop) {
        debug!("on_pipe_connected");
        self.pipe.open(event_loop);
    }

    pub fn on_pipe_ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        debug!("on_pipe_ready");
        self.pipe.ready(event_loop, events);
    }

    pub fn remove(self) -> Pipe {
        self.pipe
    }
}