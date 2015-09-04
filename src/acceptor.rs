// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use mio;

use global;
use transport::{ Listener, Connection };
use EventLoop;

pub struct Acceptor {
    token: mio::Token,
    addr: String, 
    listener: Box<Listener>
}

impl Acceptor {
    pub fn new(token: mio::Token, addr: String, listener: Box<Listener>) -> Acceptor {
        Acceptor { 
            token: token,
            addr: addr,
            listener: listener 
        }
    }

    pub fn open(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        let io = self.listener.as_evented();
        let interest = mio::EventSet::error() | mio::EventSet::readable();

        event_loop.register(io, self.token, interest, mio::PollOpt::edge())
    }

    pub fn close(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        event_loop.deregister(self.listener.as_evented())
    }

    pub fn ready(&mut self, _: &mut EventLoop, events: mio::EventSet) -> io::Result<Vec<Box<Connection>>> {
        if events.is_readable() {
            self.listener.accept()
        } else {
            Err(global::other_io_error("tcp listener ready but not readable"))
        }
    }

    pub fn addr(self) -> String {
        self.addr
    }
}