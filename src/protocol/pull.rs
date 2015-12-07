// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use super::Protocol;
use super::clear_timeout;
use super::priolist::*;
use pipe::*;
use global::*;
use event_loop_msg::{ SocketNotify, SocketOption };
use EventLoop;
use Message;

pub struct Pull {
    notify_sender: Rc<Sender<SocketNotify>>,
    recv_timeout: Option<mio::Timeout>,
    pipes: HashMap<mio::Token, Pipe>,
    fq: PrioList
}

impl Push {
    pub fn new(notify_tx: Rc<Sender<SocketNotify>>) -> Push {
        Push {
            notify_sender: notify_tx,
            recv_timeout: None,
            pipes: HashMap::new(),
            fq: PrioList::new()
        }
    }

    fn send_notify(&self, evt: SocketNotify) {
        let send_res = self.notify_sender.send(evt);

        if send_res.is_err() {
            error!("Failed to send notify to the facade: '{:?}'", send_res.err());
        }
    }
}

impl Protocol for Push {

    fn id(&self) -> u16 {
        SocketType::Pull.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Push.id()
    }

    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        match self.pipes.insert(tok, pipe) {
            None    => Ok(()),
            Some(_) => Err(invalid_data_io_error("option not supported by protocol"))
        }
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        self.fq.remove(&tok);
        self.pipes.remove(&tok)
    }

    fn register_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.fq.insert(tok, 8);
        self.pipes.get_mut(&tok).map(|p| p.register(event_loop));
    }

    fn on_pipe_register(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.pipes.get_mut(&tok).map(|p| p.on_register(event_loop));
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Option<mio::Timeout>) {
        self.send_timeout = timeout;

        if let Some(tok) = self.fq.get() {
            self.pipes.get_mut(&tok).map(|p| p.send(event_loop, Rc::new(msg)));
        }
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, _: mio::Token) {
        self.send_notify(SocketNotify::MsgSent);
        self.fq.advance();

        clear_timeout(event_loop, self.send_timeout.take());
    }

    fn on_send_timeout(&mut self, _: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.send_timeout = None;
        self.send_notify(SocketNotify::MsgNotSent(err));
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) {
        self.recv_timeout = timeout;

        if let Some(tok) = self.fq.get() {
            self.pipes.get_mut(&tok).map(|p| p.recv(event_loop));
        }
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, _: mio::Token, msg: Message) {
        self.send_notify(SocketNotify::MsgRecv(msg));
        self.fq.advance();

        clear_timeout(event_loop, self.recv_timeout.take());
    }

    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        // TODO cancel any pending pipe operation

        self.recv_timeout = None;
        self.send_notify(SocketNotify::MsgNotRecv(err));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if events.is_readable() {
            self.fq.activate(tok);
        }

        self.pipes.get_mut(&tok).map(|p| p.ready(event_loop, events));
    }

    fn set_option(&mut self, _: &mut EventLoop, _: SocketOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {}
    fn on_request_timeout(&mut self, _: &mut EventLoop) {}
}