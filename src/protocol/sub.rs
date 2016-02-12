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

use super::{ Protocol, Timeout };
use super::clear_timeout;
use super::priolist::*;
use pipe::*;
use global::*;
use event_loop_msg::{ SocketNotify, SocketOption };
use EventLoop;
use Message;

pub struct Sub {
    id: SocketId,
    body: Body,
    state: Option<State>
}

struct Body {
    notify_sender: Rc<Sender<SocketNotify>>,
    pipes: HashMap<mio::Token, Pipe>,
    fq: PrioList,
    subscriptions: HashSet<Vec<u8>>
}

enum State {
    Idle,
    Receiving(Timeout),
    RecvOnHold(Timeout)
}

impl Sub {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Sub {
        let body = Body {
            notify_sender: notify_tx,
            pipes: HashMap::new(),
            fq: PrioList::new(),
            subscriptions: HashSet::new()
        };

        Sub {
            id: socket_id,
            body: body,
            state: Some(State::Idle)
        }
    }

    fn on_state_transition<F>(&mut self, transition: F) where F : FnOnce(State, &mut Body) -> State {
        if let Some(old_state) = self.state.take() {
            let old_name = old_state.name();
            let new_state = transition(old_state, &mut self.body);
            let new_name = new_state.name();

            self.state = Some(new_state);

            debug!("[{:?}] switch from '{}' to '{}'.", self.id, old_name, new_name);
        }
    }
}

impl Protocol for Sub {
    fn id(&self) -> u16 {
        SocketType::Sub.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Pub.id()
    }

    fn add_pipe(&mut self, tok: mio::Token, pipe: Pipe) -> io::Result<()> {
        let res = self.body.add_pipe(tok, pipe);

        if res.is_ok() {
            self.on_state_transition(|s, body| s.on_pipe_added(body, tok));
        }

        res
     }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        let pipe = self.body.remove_pipe(tok);

        if pipe.is_some() {
            self.on_state_transition(|s, body| s.on_pipe_removed(body, tok));
        }

        pipe
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.on_state_transition(|s, body| s.open_pipe(body, event_loop, tok));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.on_state_transition(|s, body| s.on_pipe_opened(body, event_loop, tok));
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Timeout) {
        self.on_state_transition(|s, body| s.send(body, event_loop, Rc::new(msg), timeout));
    }

    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.on_state_transition(|s, body| s.on_send_by_pipe(body, event_loop, tok));
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s, body| s.on_send_timeout(body, event_loop));
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.on_state_transition(|s, body| s.recv(body, event_loop, timeout));
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token, msg: Message) {
        if self.body.accept(&msg) {
            self.on_state_transition(|s, body| s.on_recv_by_pipe(body, event_loop, tok, msg));
        }
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s, body| s.on_recv_timeout(body, event_loop));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.on_state_transition(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn set_option(&mut self, _: &mut EventLoop, option: SocketOption) -> io::Result<()> {
        match option {
            SocketOption::Subscribe(subscription)   => Ok(self.body.subscribe(subscription)),
            SocketOption::Unsubscribe(subscription) => Ok(self.body.unsubscribe(subscription)),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
        }
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle             => "Idle",
            State::Receiving(_)     => "Receiving",
            State::RecvOnHold(_)    => "RecvOnHold"
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
            other @ _           => other
        }
    }

    fn send(self, body: &mut Body, _: &mut EventLoop, _: Rc<Message>, _: Option<mio::Timeout>) -> State {
        let err = other_io_error("send not supported by protocol");
        let ntf = SocketNotify::MsgNotSent(err);

        body.send_notify(ntf);

        self
    }

    fn on_send_by_pipe(self, _: &mut Body, _: &mut EventLoop, _: mio::Token) -> State {
        self
    }

    fn on_send_timeout(self, _: &mut Body, _: &mut EventLoop) -> State {
        self
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        if let Some(pipe) = body.get_active_pipe() {
            pipe.recv(event_loop);

            return State::Receiving(timeout);
        } else {
            return State::RecvOnHold(timeout);
        }
    }

    fn on_recv_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, _: mio::Token, msg: Message) -> State {
        if let State::Receiving(timeout) = self {
            body.send_notify(SocketNotify::MsgRecv(msg));
            body.advance_pipe();
            clear_timeout(event_loop, timeout);
        }

        State::Idle
    }

    fn on_recv_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        body.send_notify(SocketNotify::MsgNotRecv(err));
        body.get_active_pipe().map(|p| p.cancel_recv(event_loop));
        body.advance_pipe();

        State::Idle
    }

    fn ready(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) -> State {
        body.get_pipe(tok).map(|p| p.ready(event_loop, events));

        self
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
        self.fq.remove(&tok);
        self.pipes.remove(&tok)
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.fq.insert(tok, 8);
        self.pipes.get_mut(&tok).map(|p| p.open(event_loop));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.fq.activate(tok);
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

    fn advance_pipe(&mut self) {
        self.fq.advance();
    }

    fn get_pipe<'a>(&'a mut self, tok: mio::Token) -> Option<&'a mut Pipe> {
        self.pipes.get_mut(&tok)
    }

    fn subscribe(&mut self, subscription: String) {
        self.subscriptions.insert(subscription.into_bytes());
    }

    fn unsubscribe(&mut self, subscription: String) {
        self.subscriptions.remove(&subscription.into_bytes());
    }

    fn accept(&self, msg: &Message) -> bool {
        for subscription in self.subscriptions.iter() {
            if msg.body.starts_with(subscription) {
                return true;
            }
        }

        return false;
    }
}