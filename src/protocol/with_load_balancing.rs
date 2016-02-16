// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use super::priolist::*;
use pipe::Pipe;
use global::*;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;
use super::with_pipes::WithPipes;

pub trait WithLoadBalancing : WithPipes {
    fn get_load_balancer<'a>(&'a self) -> &'a PrioList;
    fn get_load_balancer_mut<'a>(&'a mut self) -> &'a mut PrioList;

    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        match self.get_pipes_mut().insert(tok, pipe) {
            None    => Ok(()),
            Some(_) => Err(invalid_data_io_error("A pipe has already been added with that token"))
        }
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        self.get_load_balancer_mut().remove(&tok);
        self.get_pipes_mut().remove(&tok)
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_pipe_mut(&tok).map(|p| p.open(event_loop));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_load_balancer_mut().insert(tok, 8);
        self.get_pipe_mut(&tok).map(|p| p.on_open_ack(event_loop));
    }

    fn get_active_pipe<'a>(&'a mut self) -> Option<&'a mut Pipe> {
        match self.get_load_balancer().get() {
            Some(tok) => self.get_pipe_mut(&tok),
            None      => None
        }
    }

    fn is_active_pipe(&self, tok: mio::Token) -> bool {
        self.get_load_balancer().get() == Some(tok)
    }

    fn advance_pipe(&mut self, event_loop: &mut EventLoop) {
        self.get_active_pipe().map(|p| p.resync_readiness(event_loop));
        self.get_load_balancer_mut().advance();
    }

    #[cfg(not(windows))]
    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if events.is_writable() {
            self.get_load_balancer_mut().activate(tok);
        } else {
            self.get_load_balancer_mut().deactivate(tok);
        }
        
        self.get_pipe_mut(&tok).map(|p| p.ready(event_loop, events));
    }

    #[cfg(windows)]
    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if events.is_writable() {
            self.get_load_balancer_mut().activate(tok);
        }

        self.get_pipe_mut(&tok).map(|p| p.ready(event_loop, events));
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> bool {
        self.get_active_pipe().map(|p| p.send(event_loop, msg)).is_some()
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgSent);
        self.advance_pipe(event_loop);

        clear_timeout(event_loop, timeout);
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.send_notify(SocketNotify::MsgNotSent(err));
        self.get_active_pipe().map(|p| p.cancel_send(event_loop));
        self.advance_pipe(event_loop);
    }
}
