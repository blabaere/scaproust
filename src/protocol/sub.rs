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
use super::priolist::*;
use super::with_fair_queue::WithFairQueue;
use super::without_send::WithoutSend;
use super::with_pipes::WithPipes;
use super::with_notify::WithNotify;
use pipe::Pipe;
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
    Receiving(mio::Token, Timeout),
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

impl Protocol for Sub {
    fn get_type(&self) -> SocketType {
        SocketType::Sub
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
        self.apply(|s, body| s.send(body, event_loop, Rc::new(msg), timeout));
    }

    fn on_send_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.apply(|s, body| s.on_send_done(body, event_loop, tok));
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_send_timeout(body, event_loop));
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.apply(|s, body| s.recv(body, event_loop, timeout));
    }

    fn on_recv_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token, msg: Message) {
        self.apply(|s, body| s.on_recv_done(body, event_loop, tok, msg));
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_recv_timeout(body, event_loop));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.apply(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn set_option(&mut self, _: &mut EventLoop, option: SocketOption) -> io::Result<()> {
        match option {
            SocketOption::Subscribe(subscription)   => Ok(self.body.subscribe(subscription)),
            SocketOption::Unsubscribe(subscription) => Ok(self.body.unsubscribe(subscription)),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
        }
    }

    fn destroy(&mut self, event_loop: &mut EventLoop) {
        self.body.destroy_pipes(event_loop);
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle           => "Idle",
            State::Receiving(_,_) => "Receiving",
            State::RecvOnHold(_)  => "RecvOnHold"
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, _: &mut Body, tok: mio::Token) -> State {
        match self {
            State::Receiving(token, timeout) => {
                if token == tok {
                    State::RecvOnHold(timeout)
                } else {
                    State::Receiving(token, timeout)
                }
            },
            other => other
        }
    }

    fn open_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.open_pipe(event_loop, tok);

        self
    }

    fn on_pipe_opened(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.on_pipe_opened(event_loop, tok);

        self
    }

    fn send(self, body: &mut Body, _: &mut EventLoop, _: Rc<Message>, _: Option<mio::Timeout>) -> State {
        body.send();

        self
    }

    fn on_send_done(self, _: &mut Body, _: &mut EventLoop, _: mio::Token) -> State {
        self
    }

    fn on_send_timeout(self, _: &mut Body, _: &mut EventLoop) -> State {
        self
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        try_recv(body, event_loop, timeout)
    }

    fn on_recv_done(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, msg: Message) -> State {
        match self {
            State::Receiving(token, timeout) => {
                if tok == token {
                    if body.accept(&msg) {
                        body.on_recv_done(event_loop, msg, timeout);
                        State::Idle
                    } else {
                        try_recv(body, event_loop, timeout)
                    }
                } else {
                    State::Receiving(token, timeout)
                }
            }
            other => other
        }
    }

    fn on_recv_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        body.on_recv_timeout(event_loop);

        State::Idle
    }

    fn ready(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) -> State {
        body.ready(event_loop, tok, events);

        match self {
            State::RecvOnHold(t) => State::Idle.recv(body, event_loop, t),
            other                => other
        }
    }
}

fn try_recv(body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
    if let Some(tok) = body.recv_from(event_loop) {
        State::Receiving(tok, timeout)
    } else {
        State::RecvOnHold(timeout)
    }
}

impl Body {
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

        false
    }
}

impl WithNotify for Body {
    fn get_notify_sender(&self) -> &Sender<SocketNotify> {
        &self.notify_sender
    }
}

impl WithPipes for Body {
    fn get_pipes(&self) -> &HashMap<mio::Token, Pipe> {
        &self.pipes
    }

    fn get_pipes_mut(&mut self) -> &mut HashMap<mio::Token, Pipe> {
        &mut self.pipes
    }
}

impl WithFairQueue for Body {

    fn get_fair_queue(&self) -> &PrioList {
        &self.fq
    }

    fn get_fair_queue_mut(&mut self) -> &mut PrioList {
        &mut self.fq
    }
}

impl WithoutSend for Body {
}