// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use super::excl::*;
use pipe::Pipe;
use global::*;
use event_loop_msg::{ SocketNotify };
use EventLoop;
use Message;

pub struct Pair {
    id: SocketId,
    body: Body,
    state: Option<State>
}

struct Body {
    notify_sender: Rc<Sender<SocketNotify>>,
    excl: Excl
}

enum State {
    Idle,
    Sending(Rc<Message>, Timeout),
    SendOnHold(Rc<Message>, Timeout),
    Receiving(Timeout),
    RecvOnHold(Timeout)
}

impl Pair {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Pair {
        let body = Body {
            notify_sender: notify_tx,
            excl: Excl::new()
        };

        Pair {
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

impl Protocol for Pair {
    fn get_type(&self) -> SocketType {
        SocketType::Pair
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
        self.on_state_transition(|s, body| s.on_recv_by_pipe(body, event_loop, tok, msg));
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s, body| s.on_recv_timeout(body, event_loop));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.on_state_transition(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn destroy(&mut self, event_loop: &mut EventLoop) {
        self.body.destroy(event_loop);
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle             => "Idle",
            State::Sending(_, _)    => "Sending",
            State::SendOnHold(_, _) => "SendOnHold",
            State::Receiving(_)     => "Receiving",
            State::RecvOnHold(_)    => "RecvOnHold"
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, _: &mut Body, _: mio::Token) -> State {
        match self {
            State::Sending(msg, t) => State::SendOnHold(msg, t),
            State::Receiving(t)    => State::RecvOnHold(t),
            other @ _              => other
        }
    }

    fn open_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.open_pipe(event_loop, tok);

        self
    }

    fn on_pipe_opened(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.on_pipe_opened(event_loop, tok);

        match self {
            State::SendOnHold(msg, t) => State::Idle.send(body, event_loop, msg, t),
            State::RecvOnHold(t)      => State::Idle.recv(body, event_loop, t),
            other @ _                 => other
        }
    }

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Option<mio::Timeout>) -> State {
        if let Some(pipe) = body.get_active_pipe() {
            pipe.send(event_loop, msg.clone());

            return State::Sending(msg, timeout);
        } else {
            return State::SendOnHold(msg, timeout);
        }
    }

    fn on_send_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, _: mio::Token) -> State {
        if let State::Sending(_, timeout) = self {
            body.send_notify(SocketNotify::MsgSent);
            clear_timeout(event_loop, timeout);
        }

        State::Idle
    }

    fn on_send_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        body.send_notify(SocketNotify::MsgNotSent(err));
        body.get_active_pipe().map(|p| p.cancel_send(event_loop));

        State::Idle
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
            clear_timeout(event_loop, timeout);
        }

        State::Idle
    }

    fn on_recv_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        body.send_notify(SocketNotify::MsgNotRecv(err));
        body.get_active_pipe().map(|p| p.cancel_recv(event_loop));

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
         if self.excl.add(tok, pipe) {
            Ok(())
         } else {
            Err(other_io_error("supports only one item"))
         }
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        self.excl.remove(tok)
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.excl.get(tok).map(|p| p.open(event_loop));
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.excl.activate(tok);
        self.excl.get(tok).map(|p| p.on_open_ack(event_loop));
    }

    fn get_active_pipe<'a>(&'a mut self) -> Option<&'a mut Pipe> {
        self.excl.get_active()
    }

    fn get_pipe<'a>(&'a mut self, tok: mio::Token) -> Option<&'a mut Pipe> {
        self.excl.get(tok)
    }

    fn destroy(&mut self, event_loop: &mut EventLoop) {
        self.excl.get_pipe().map(|p| p.close(event_loop));
    }
}