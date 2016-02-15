// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use super::priolist::*;
use super::with_fair_queue::WithFairQueue;
use super::with_pipes::WithPipes;
use super::with_unicast_send::WithUnicastSend;
use super::with_notify::WithNotify;
use super::with_backtrace::WithBacktrace;
use pipe::Pipe;
use global::*;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;

pub struct Resp {
    id: SocketId,
    body: Body,
    state: Option<State>
}

struct Body {
    notify_sender: Rc<Sender<SocketNotify>>,
    pipes: HashMap<mio::Token, Pipe>,
    fq: PrioList,
    ttl: u8,
    backtrace: Vec<u8>
}

enum State {
    Idle,
    Receiving(Timeout),
    RecvOnHold(Timeout),
    Active(mio::Token),
    Sending(Rc<Message>, Timeout, mio::Token),
    SendOnHold(Rc<Message>, Timeout, mio::Token)
}

impl Resp {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Resp {
        let body = Body {
            notify_sender: notify_tx,
            pipes: HashMap::new(),
            fq: PrioList::new(),
            ttl: 8,
            backtrace: Vec::with_capacity(64)
        };

        Resp {
            id: socket_id,
            body: body,
            state: Some(State::Idle)
        }
    }

    fn apply<F>(&mut self, transition: F) where F : FnOnce(State, &mut Body) -> State {
        if let Some(old_state) = self.state.take() {
            let old_name = old_state.name();
            let new_state = transition(old_state, &mut self.body);
            let new_name = new_state.name();

            self.state = Some(new_state);

            debug!("[{:?}] switch from '{}' to '{}'.", self.id, old_name, new_name);
        }
    }
}

impl Protocol for Resp {
    fn get_type(&self) -> SocketType {
        SocketType::Respondent
    }

    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        let res = self.body.add_pipe(tok, pipe);

        if res.is_ok() {
            self.apply(|s, body| s.on_pipe_added(body, tok));
        }

        res
     }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        let pipe = self.body.remove_pipe(tok);

        if pipe.is_some() {
            self.apply(|s, body| s.on_pipe_removed(body, tok));
        }

        pipe
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.apply(|s, body| s.open_pipe(body, event_loop, tok));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.apply(|s, body| s.on_pipe_opened(body, event_loop, tok));
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Timeout) {
        let raw_msg = encode(msg, self.body.backtrace());

        self.apply(|s, body| s.send(body, event_loop, Rc::new(raw_msg), timeout));
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.body.clear_backtrace();

        self.apply(|s, body| s.on_send_by_pipe(body, event_loop, tok));
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_send_timeout(body, event_loop));
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.apply(|s, body| s.recv(body, event_loop, timeout));
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token, raw_msg: Message) {
        if let Some(msg) = decode(raw_msg, tok, self.body.ttl) {
            self.body.set_backtrace(&msg.header);

            self.apply(|s, body| s.on_recv_by_pipe(body, event_loop, tok, msg));
        }
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_recv_timeout(body, event_loop));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.apply(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn destroy(&mut self, event_loop: &mut EventLoop) {
        self.body.destroy_pipes(event_loop);
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle              => "Idle",
            State::Sending(_,_,_)    => "Sending",
            State::SendOnHold(_,_,_) => "SendOnHold",
            State::Active(_)         => "Active",
            State::Receiving(_)      => "Receiving",
            State::RecvOnHold(_)     => "RecvOnHold"
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn open_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.open_pipe(event_loop, tok);

        self
    }

    fn on_pipe_opened(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.on_pipe_opened(event_loop, tok);

        match self {
            State::SendOnHold(msg, timeout, t) => {
                if t == tok {
                    try_send(body, event_loop, msg, timeout, tok)
                } else {
                    State::SendOnHold(msg, timeout, t)
                }
            },
            other @ _ => other
        }
    }

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Option<mio::Timeout>) -> State {
        if let State::Active(tok) = self {
            try_send(body, event_loop, msg, timeout, tok)
        } else {
            let err = other_io_error("Can't send: currently no pending survey");
            body.send_notify(SocketNotify::MsgNotSent(err));
            clear_timeout(event_loop, timeout);

            self
        }
    }

    fn on_send_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, _: mio::Token) -> State {
        match self {
            State::Sending(_, timeout, _)    => body.on_send_by_pipe(event_loop, timeout),
            State::SendOnHold(_, timeout, _) => body.on_send_by_pipe(event_loop, timeout),
            _ => {}
        }

        State::Idle
    }

    fn on_send_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        match self {
            State::Sending(_, _, t)    => body.on_send_timeout(event_loop, t),
            State::SendOnHold(_, _, t) => body.on_send_timeout(event_loop, t),
            _ => {}
        }

        State::Idle
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        if body.recv(event_loop) {
            State::Receiving(timeout)
        } else {
            State::RecvOnHold(timeout)
        }
    }

    fn on_recv_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, msg: Message) -> State {
        if let State::Receiving(timeout) = self {
            body.on_recv_by_pipe(event_loop, msg, timeout);
        }

        State::Active(tok)
    }

    fn on_recv_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        body.on_recv_timeout(event_loop);

        State::Idle
    }

    fn ready(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) -> State {
        body.ready(event_loop, tok, events);

        match self {
            State::RecvOnHold(t) => State::Idle.recv(body, event_loop, t),
            other @ _            => other
        }
    }
}

fn try_send(body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Option<mio::Timeout>, tok: mio::Token) -> State {
    if body.send(event_loop, msg.clone(), tok) {
        State::Sending(msg, timeout, tok)
    } else {
        State::SendOnHold(msg, timeout, tok)
    }
}

impl WithBacktrace for Body {
    fn get_backtrace<'a>(&'a self) -> &'a Vec<u8> {
        &self.backtrace
    }

    fn get_backtrace_mut<'a>(&'a mut self) -> &'a mut Vec<u8> {
        &mut self.backtrace
    }
}

impl WithNotify for Body {
    fn get_notify_sender<'a>(&'a self) -> &'a Sender<SocketNotify> {
        &self.notify_sender
    }
}

impl WithPipes for Body {
    fn get_pipes<'a>(&'a self) -> &'a HashMap<mio::Token, Pipe> {
        &self.pipes
    }

    fn get_pipes_mut<'a>(&'a mut self) -> &'a mut HashMap<mio::Token, Pipe> {
        &mut self.pipes
    }
}

impl WithFairQueue for Body {
    fn get_fair_queue<'a>(&'a self) -> &'a PrioList {
        &self.fq
    }

    fn get_fair_queue_mut<'a>(&'a mut self) -> &'a mut PrioList {
        &mut self.fq
    }
}

impl WithUnicastSend for Body {
}

fn decode(raw_msg: Message, _: mio::Token, ttl: u8) -> Option<Message> {
    let (mut header, mut body) = raw_msg.explode();
    let mut hops = 0;

    loop {
        if hops >= ttl {
            return None;
        }
        hops += 1;

        if body.len() < 4 {
            return None;
        }

        let tail = body.split_off(4);
        header.extend_from_slice(&body);

        let position = header.len() - 4;
        if header[position] & 0x80 != 0 {
            return Some(Message::with_header_and_body(header, tail));
        }
        body = tail;
    }
}

fn encode(msg: Message, backtrace: &[u8]) -> Message {
    let (mut header, body) = msg.explode();

    header.extend_from_slice(backtrace);

    Message::with_header_and_body(header, body)
}
