// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use protocol::policy::{ Timeout, clear_timeout };
use protocol::policy::priolist::*;
use transport::pipe::Pipe;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;
use super::WithPipes;

pub trait WithLoadBalancing : WithPipes {
    fn get_load_balancer(&self) -> &PrioList;
    fn get_load_balancer_mut(&mut self) -> &mut PrioList;

    fn insert_into_load_balancer(&mut self, tok: mio::Token, priority: u8) {
        self.get_load_balancer_mut().insert(tok, priority)
    }

    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        let priority = pipe.get_send_priority();

        self.insert_into_pipes(tok, pipe).map(|_| self.insert_into_load_balancer(tok, priority))
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        self.get_load_balancer_mut().remove(&tok);
        self.get_pipes_mut().remove(&tok)
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_pipe_mut(&tok).map(|p| p.open(event_loop));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_load_balancer_mut().show(tok);
        self.get_pipe_mut(&tok).map(|p| p.on_open_ack(event_loop));
    }

    fn get_active_pipe(&mut self) -> Option<&mut Pipe> {
        match self.get_load_balancer().get() {
            Some(tok) => self.get_pipe_mut(&tok),
            None      => None
        }
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if events.is_writable() {
            self.get_load_balancer_mut().activate(tok);
        } else {
            self.get_load_balancer_mut().deactivate(tok);
        }
        
        self.get_pipe_mut(&tok).map(|p| p.ready(event_loop, events));
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> bool {
        self.get_active_pipe().map(|p| p.send(event_loop, msg)).is_some()
    }

    fn send_to(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> Option<mio::Token> {
        self.get_active_pipe().map(|p| {
            p.send(event_loop, msg);
            p.token()
        })
    }

    fn on_send_done(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgSent);
        self.get_active_pipe().map(|p| p.resync_readiness(event_loop));
        self.get_load_balancer_mut().advance();

        clear_timeout(event_loop, timeout);
    }

    fn on_send_done_late(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_load_balancer_mut().show(tok);
        self.get_pipe_mut(&tok).map(|p| p.resync_readiness(event_loop));
    }

    fn on_send_timeout(&mut self) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.send_notify(SocketNotify::MsgNotSent(err));
        self.get_load_balancer_mut().skip();
    }
}
