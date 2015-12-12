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

pub struct Push {
    notify_sender: Rc<Sender<SocketNotify>>,
    send_timeout: Option<mio::Timeout>,
    pipes: HashMap<mio::Token, Pipe>,
    lb: PrioList,
    pending_send: Option<Rc<Message>>,
    pending_send_to: Option<mio::Token>
}

impl Push {
    pub fn new(notify_tx: Rc<Sender<SocketNotify>>) -> Push {
        Push {
            notify_sender: notify_tx,
            send_timeout: None,
            pipes: HashMap::new(),
            lb: PrioList::new(),
            pending_send: None,
            pending_send_to: None
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
        SocketType::Push.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Pull.id()
    }

    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        match self.pipes.insert(tok, pipe) {
            None    => Ok(()),
            Some(_) => Err(invalid_data_io_error("A pipe has already been added with that token"))
        }
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        // TODO check if we were not sending via this pipe
        self.lb.remove(&tok);
        self.pipes.remove(&tok)
    }

    fn register_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.lb.insert(tok, 8);
        self.pipes.get_mut(&tok).map(|p| p.register(event_loop));
    }

    fn on_pipe_register(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.lb.activate(tok);
        self.pipes.get_mut(&tok).map(|p| p.on_register(event_loop));

        /*if self.pending_send.is_some() && self.pending_send_to.is_none() {
            let msg = self.pending_send.as_ref()
        }*/
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Option<mio::Timeout>) {
        let msg = Rc::new(msg);

        self.send_timeout = timeout;

        if let Some(tok) = self.lb.get() {
            self.pending_send = Some(msg.clone());
            self.pending_send_to = Some(tok);
            self.pipes.get_mut(&tok).map(|p| p.send(event_loop, msg));
        } else {
            self.pending_send = Some(msg);
            self.pending_send_to = None;
        }
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, _: mio::Token) {
        self.send_notify(SocketNotify::MsgSent);
        self.lb.advance();
        self.pending_send = None;
        self.pending_send_to = None;

        clear_timeout(event_loop, self.send_timeout.take());
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        if let Some(tok) = self.pending_send_to.take() {
            self.pipes.get_mut(&tok).map(|p| p.cancel_send(event_loop));
        }

        self.send_timeout = None;
        self.pending_send = None;
        self.pending_send_to = None;
        self.send_notify(SocketNotify::MsgNotSent(err));
    }

    fn recv(&mut self, _: &mut EventLoop, _: Option<mio::Timeout>) {
        let err = other_io_error("recv not supported by protocol");
        let ntf = SocketNotify::MsgNotRecv(err);

        self.send_notify(ntf);
    }

    fn on_recv_by_pipe(&mut self, _: &mut EventLoop, _: mio::Token, _: Message) {
    }

    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.pipes.get_mut(&tok).map(|p| p.ready(event_loop, events));
    }

    fn set_option(&mut self, _: &mut EventLoop, _: SocketOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {}
    fn on_request_timeout(&mut self, _: &mut EventLoop) {}
}