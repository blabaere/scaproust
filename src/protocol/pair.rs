// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use super::Protocol;
use super::clear_timeout;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::{ SocketNotify, SocketOption };
use EventLoop;
use Message;

pub struct Pair {
    notify_sender: Rc<Sender<SocketNotify>>,
    send_timeout: Option<mio::Timeout>,
    recv_timeout: Option<mio::Timeout>,
    endpoint: Option<Endpoint>
}

impl Pair {
    pub fn new(notify_tx: Rc<Sender<SocketNotify>>) -> Pair {
        Pair {
            notify_sender: notify_tx,
            send_timeout: None,
            recv_timeout: None,
            endpoint: None
        }
    }

    fn send_notify(&self, evt: SocketNotify) {
        let send_res = self.notify_sender.send(evt);

        if send_res.is_err() {
            error!("Failed to send notify to the facade: '{:?}'", send_res.err());
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

    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) -> io::Result<()> {
        self.endpoint = Some(Endpoint::new(token, pipe));
        Ok(())
    }

    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
        if Some(token) == self.endpoint.as_ref().map(|e| e.token()) {
            self.endpoint.take().map(|e| e.remove())
        } else {
            None
        }
    }

    fn register_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            if endpoint.token() == tok {
                endpoint.register_pipe(event_loop);
            }
        }
    }

    fn on_pipe_register(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            if endpoint.token() == tok {
                endpoint.on_pipe_register(event_loop);
            }
        }
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Option<mio::Timeout>) {
        self.send_timeout = timeout;

        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.send(event_loop, Rc::new(msg));
        }
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, _: mio::Token) {
        self.send_notify(SocketNotify::MsgSent);

        clear_timeout(event_loop, self.send_timeout.take());
    }

    fn on_send_timeout(&mut self, _: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        // TODO cancel any pending pipe operation

        self.send_timeout = None;
        self.send_notify(SocketNotify::MsgNotSent(err));
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) {
        self.recv_timeout = timeout;

        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.recv(event_loop);
        }
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, _: mio::Token, msg: Message) {
        self.send_notify(SocketNotify::MsgRecv(msg));

        clear_timeout(event_loop, self.recv_timeout.take());
    }

    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        // TODO cancel any pending pipe operation

        self.recv_timeout = None;
        self.send_notify(SocketNotify::MsgNotRecv(err));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if let Some(endpoint) = self.endpoint.as_mut() {
            if endpoint.token() == tok {
                endpoint.on_pipe_ready(event_loop, events);
            }
        }
    }

    fn set_option(&mut self, _: &mut EventLoop, _: SocketOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {}
    fn on_request_timeout(&mut self, _: &mut EventLoop) {}
}