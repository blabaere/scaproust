// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

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

pub trait WithFairQueue : WithPipes {
    fn get_fair_queue<'a>(&'a self) -> &'a PrioList;
    fn get_fair_queue_mut<'a>(&'a mut self) -> &'a mut PrioList;
    
    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        match self.get_pipes_mut().insert(tok, pipe) {
            None    => Ok(()),
            Some(_) => Err(invalid_data_io_error("A pipe has already been added with that token"))
        }
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        self.get_fair_queue_mut().remove(&tok);
        self.get_pipes_mut().remove(&tok)
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_pipe_mut(&tok).map(|p| p.open(event_loop));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        let priority = self.get_pipe(&tok).map(|p| p.get_recv_priority()).unwrap_or(8);

        self.get_fair_queue_mut().insert(tok, priority);
        self.get_pipe_mut(&tok).map(|p| p.on_open_ack(event_loop));
    }

    fn get_active_pipe<'a>(&'a self) -> Option<&'a Pipe> {
        match self.get_fair_queue().get() {
            Some(tok) => self.get_pipe(&tok),
            None      => None
        }
    }

    fn get_active_pipe_mut<'a>(&'a mut self) -> Option<&'a mut Pipe> {
        match self.get_fair_queue().get() {
            Some(tok) => self.get_pipe_mut(&tok),
            None      => None
        }
    }

    fn has_active_pipe(&self) -> bool {
        match self.get_fair_queue().get() {
            Some(tok) => self.get_pipes().get(&tok).is_some(),
            None      => false
        }
    }

    fn is_active_pipe(&self, tok: mio::Token) -> bool {
        self.get_fair_queue().get() == Some(tok)
    }

    fn advance_pipe(&mut self, event_loop: &mut EventLoop) {
        self.get_active_pipe_mut().map(|p| p.resync_readiness(event_loop));
        self.get_fair_queue_mut().advance();
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if events.is_readable() {
            self.get_fair_queue_mut().activate(tok);
        } else {
            self.get_fair_queue_mut().deactivate(tok);
        }

        self.get_pipe_mut(&tok).map(|p| p.ready(event_loop, events));
    }

    fn recv(&mut self, event_loop: &mut EventLoop) -> bool {
        self.get_active_pipe_mut().map(|p| p.recv(event_loop)).is_some()
    }

    fn recv_from(&mut self, event_loop: &mut EventLoop) -> Option<mio::Token> {
        self.get_active_pipe_mut().map(|p| {
            p.recv(event_loop);
            p.token()
        })
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgRecv(msg));
        self.advance_pipe(event_loop);

        clear_timeout(event_loop, timeout);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.send_notify(SocketNotify::MsgNotRecv(err));
        self.get_active_pipe_mut().map(|p| p.cancel_recv(event_loop));
        self.advance_pipe(event_loop);
    }

    fn can_recv(&self) -> bool {
        match self.get_active_pipe() {
            Some(pipe) => pipe.can_recv(),
            None       => false,
        }
    }
}
