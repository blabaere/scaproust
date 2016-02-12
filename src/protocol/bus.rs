// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::{ HashMap, HashSet };
use std::sync::mpsc::Sender;
use std::io;

use mio;

use byteorder::*;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use super::priolist2::*;
use pipe::*;
use global::*;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;

pub struct Bus {
    id: SocketId,
    body: Body,
    state: Option<State>
}

struct Body {
    notify_sender: Rc<Sender<SocketNotify>>,
    pipes: HashMap<mio::Token, Pipe>,
    dist: HashSet<mio::Token>,
    fq: PrioList
}

enum State {
    Idle,
    Receiving(Timeout),
    RecvOnHold(Timeout)
}

impl Bus {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Bus {
        let body = Body {
            notify_sender: notify_tx,
            pipes: HashMap::new(),
            dist: HashSet::new(),
            fq: PrioList::new()
        };

        Bus {
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

impl Protocol for Bus {
    fn id(&self) -> u16 {
        SocketType::Bus.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Bus.id()
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
        let (raw_msg, pipe_id) = encode(msg);
        let origin = pipe_id.map(|id| mio::Token(id as usize));

        self.apply(|s, body| s.send(body, event_loop, Rc::new(raw_msg), origin, timeout));
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.apply(|s, body| s.on_send_by_pipe(body, event_loop, tok));
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_send_timeout(body, event_loop));
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.apply(|s, body| s.recv(body, event_loop, timeout));
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token, raw_msg: Message) {
        let msg = decode(raw_msg, tok);

        self.apply(|s, body| s.on_recv_by_pipe(body, event_loop, tok, msg));
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_recv_timeout(body, event_loop));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.apply(|s, body| s.ready(body, event_loop, tok, events));
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle          => "Idle",
            State::Receiving(_)  => "Receiving",
            State::RecvOnHold(_) => "RecvOnHold"
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, body: &mut Body, tok: mio::Token) -> State {
        match self {
            State::Receiving(t) => {
                if body.is_active_pipe(tok) {
                    State::RecvOnHold(t)
                } else {
                    State::Receiving(t)
                }
            },
            other @ _ => other
        }
    }

    fn open_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.open_pipe(event_loop, tok);

        self
    }

    fn on_pipe_opened(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.on_pipe_opened(event_loop, tok);

        match self {
            State::RecvOnHold(t) => State::Idle.recv(body, event_loop, t),
            other @ _            => other
        }
    }

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, origin: Option<mio::Token>, _: Option<mio::Timeout>) -> State {
        body.send(event_loop, msg, origin);

        State::Idle
    }

    fn on_send_by_pipe(self, _: &mut Body, _: &mut EventLoop, _: mio::Token) -> State {
        self
    }

    fn on_send_timeout(self, _: &mut Body, _: &mut EventLoop) -> State {
        self
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        if body.recv(event_loop) {
            State::Receiving(timeout)
        } else {
            State::RecvOnHold(timeout)
        }
    }

    fn on_recv_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, _: mio::Token, msg: Message) -> State {
        if let State::Receiving(timeout) = self {
            body.on_recv_by_pipe(event_loop, msg, timeout);
        }

        State::Idle
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

impl Body {

    fn send_notify(&self, evt: SocketNotify) {
        let send_res = self.notify_sender.send(evt);

        if send_res.is_err() {
            error!("Failed to send notify to the facade: '{:?}'", send_res.err());
        }
    }
    
    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        match self.pipes.insert(tok, pipe) {
            None    => Ok(()),
            Some(_) => Err(invalid_data_io_error("A pipe has already been added with that token"))
        }
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        self.dist.remove(&tok);
        self.fq.remove(&tok);
        self.pipes.remove(&tok)
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.pipes.get_mut(&tok).map(|p| p.open(event_loop));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.dist.insert(tok);
        self.fq.insert(tok, 8);
        self.pipes.get_mut(&tok).map(|p| p.on_open_ack(event_loop));
    }

    fn get_active_pipe<'a>(&'a mut self) -> Option<&'a mut Pipe> {
        match self.fq.get() {
            Some(tok) => self.pipes.get_mut(&tok),
            None      => None
        }
    }

    fn is_active_pipe(&self, tok: mio::Token) -> bool {
        self.fq.get() == Some(tok)
    }

    fn advance_pipe(&mut self, event_loop: &mut EventLoop) {
        self.get_active_pipe().map(|p| p.resync_readiness(event_loop));
        self.fq.deactivate_and_advance();
    }

    fn get_pipe<'a>(&'a mut self, tok: mio::Token) -> Option<&'a mut Pipe> {
        self.pipes.get_mut(&tok)
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if events.is_readable() {
            self.fq.activate(tok);
        }

        self.get_pipe(tok).map(|p| p.ready(event_loop, events));
    }

    fn recv(&mut self, event_loop: &mut EventLoop) -> bool {
        self.get_active_pipe().map(|p| p.recv(event_loop)).is_some()
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgRecv(msg));
        self.advance_pipe(event_loop);

        clear_timeout(event_loop, timeout);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.send_notify(SocketNotify::MsgNotRecv(err));
        self.get_active_pipe().map(|p| p.cancel_recv(event_loop));
        self.advance_pipe(event_loop);
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>, origin: Option<mio::Token>) {
        if let Some(excluded) = origin {
            self.broadcast(event_loop, msg, &mut |&tok| tok != excluded);
        } else {
            self.broadcast(event_loop, msg, &mut |_| true);
        }

        self.send_notify(SocketNotify::MsgSent);
    }

    fn broadcast<P>(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>, predicate: &mut P) where P : FnMut(&mio::Token) -> bool {
        for tok in self.dist.iter().filter(|t| predicate(t)) {
            let msg = msg.clone();

            self.pipes.get_mut(tok).map(|p| p.send_nb(event_loop, msg));
        }
    }

}

fn decode(raw_msg: Message, pipe_id: mio::Token) -> Message {
    let mut msg = raw_msg;
    let mut pipe_id_bytes: [u8; 4] = [0; 4];

    BigEndian::write_u32(&mut pipe_id_bytes[..], pipe_id.as_usize() as u32);

    msg.header.reserve(4);
    msg.header.extend_from_slice(&pipe_id_bytes);
    msg
}

fn encode(msg: Message) -> (Message, Option<u32>) {
    if msg.get_header().len() < 4 {
        return (msg, None);
    }

    let (mut header, body) = msg.explode();
    let remaining_header = header.split_off(4);
    let pipe_id = BigEndian::read_u32(&header);
    let raw_msg = Message::with_header_and_body(remaining_header, body);

    (raw_msg, Some(pipe_id))
}