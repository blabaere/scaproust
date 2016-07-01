// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use protocol::Protocol;
use protocol::policy::*;
use transport::pipe::Pipe;
use global::*;
use event_loop_msg::SocketNotify;
use EventLoop;
use Message;

pub struct Pair {
    id: SocketId,
    body: Body,
    state: Option<State>
}


struct Body {
    notify_sender: Rc<Sender<SocketNotify>>,
    pipe: Option<Pipe>,

}

enum State {
    Idle,
    Sending(mio::Token, Rc<Message>, Timeout),
    SendOnHold(Rc<Message>, Timeout),
    Receiving(mio::Token, Timeout),
    RecvOnHold(Timeout)
}

impl Pair {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Pair {
        let body = Body {
            notify_sender: notify_tx,
            pipe: None
        };

        Pair {
            id: socket_id,
            body: body,
            state: Some(State::Idle)
        }
    }

    fn on_state_transition<F>(&mut self, transition: F) where F: FnOnce(State, &mut Body) -> State {
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

    fn on_send_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.on_state_transition(|s, body| s.on_send_done(body, event_loop, tok));
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s, body| s.on_send_timeout(body, event_loop));
    }

    fn has_pending_send(&self) -> bool {
        self.state.as_ref().map_or(false, |s| s.has_pending_send())
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.on_state_transition(|s, body| s.recv(body, event_loop, timeout));
    }

    fn on_recv_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token, msg: Message) {
        self.on_state_transition(|s, body| s.on_recv_done(body, event_loop, tok, msg));
    }

    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
        self.on_state_transition(|s, body| s.on_recv_timeout(body));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.on_state_transition(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn can_recv(&self) -> bool {
        self.body.can_recv()
    }

    fn destroy(&mut self, event_loop: &mut EventLoop) {
        self.body.destroy_pipe(event_loop);
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle            => "Idle",
            State::Sending(_,_,_)  => "Sending",
            State::SendOnHold(_,_) => "SendOnHold",
            State::Receiving(_,_)  => "Receiving",
            State::RecvOnHold(_)   => "RecvOnHold",
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, _: &mut Body, tok: mio::Token) -> State {
        match self {
            State::Sending(token, msg, timeout) => {
                if token == tok {
                    State::SendOnHold(msg, timeout)
                } else {
                    State::Sending(token, msg, timeout)
                }
            },
            State::Receiving(token, timeout) => {
                if tok == token {
                    State::RecvOnHold(timeout)
                } else {
                    State::Receiving(token, timeout)
                }
            },
            other => other,
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
            State::RecvOnHold(t) => State::Idle.recv(body, event_loop, t),
            other => other,
        }
    }

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Option<mio::Timeout>) -> State {
        if let Some(tok) = body.send_to(event_loop, msg.clone()) {
            State::Sending(tok, msg, timeout)
        } else {
            State::SendOnHold(msg, timeout)
        }
    }

    fn on_send_done(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        match self {
            State::Sending(token, msg, timeout) => {
                if tok == token {
                    body.on_send_done(event_loop, tok, timeout);
                    State::Idle
                } else {
                    body.on_send_done_late(event_loop, tok);
                    State::Sending(token, msg, timeout)
                }
            }
            other => {
                body.on_send_done_late(event_loop, tok);
                other
            }
        }
    }

    fn on_send_timeout(self, body: &mut Body, _: &mut EventLoop) -> State {
        body.on_send_timeout();

        State::Idle
    }

    fn has_pending_send(&self) -> bool {
        match *self {
            State::Sending(_, _, _) |
            State::SendOnHold(_, _) => true,
            _ => false
        }
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        if let Some(tok) = body.recv_from(event_loop) {
            State::Receiving(tok, timeout)
        } else {
            State::RecvOnHold(timeout)
        }
    }

    fn on_recv_done(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, msg: Message) -> State {
        match self {
            State::Receiving(token, timeout) => {
                if tok == token {
                    body.on_recv_done(event_loop, tok, msg, timeout);
                    State::Idle
                } else {
                    body.on_recv_done_late(event_loop, tok);
                    State::Receiving(token, timeout)
                }
            }
            other => {
                body.on_recv_done_late(event_loop, tok);
                other
            }
        }
    }

    fn on_recv_timeout(self, body: &mut Body) -> State {
        body.on_recv_timeout();

        State::Idle
    }

    fn ready(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) -> State {
        body.get_pipe_mut(tok).map(|p| p.ready(event_loop, events));

        match self {
            State::SendOnHold(msg, t) => State::Idle.send(body, event_loop, msg, t),
            State::RecvOnHold(t) => State::Idle.recv(body, event_loop, t),
            other => other,
        }
    }
}

impl WithNotify for Body {
    fn get_notify_sender(&self) -> &Sender<SocketNotify> {
        &self.notify_sender
    }
}

impl Body {

    fn add_pipe(&mut self, _: mio::Token, pipe: Pipe) -> io::Result<()> {
        if self.pipe.is_some() {
            return Err(other_io_error("supports only one item"));
        }
        self.pipe = Some(pipe);
        Ok(())
    }

    fn remove_pipe(&mut self, tok: mio::Token) -> Option<Pipe> {
        if self.is_pipe(tok) {
            self.pipe.take()
        } else {
            None
        }
    }

    fn is_pipe(&self, tok: mio::Token) -> bool {
        match self.pipe.as_ref() {
            Some(pipe) => tok == pipe.token(),
            None       => false
        }
    }

    fn can_recv(&self) -> bool {
        match self.pipe.as_ref() {
            Some(pipe) => pipe.can_recv(),
            None       => false
        }
    }

    fn destroy_pipe(&mut self, event_loop: &mut EventLoop) {
        self.pipe.take().map(|mut p| p.close(event_loop));
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_pipe_mut(tok).map(|p| p.open(event_loop));
    }

    fn get_pipe_mut(&mut self, tok: mio::Token) -> Option<&mut Pipe> {
        if self.is_pipe(tok) {
            self.pipe.as_mut()
        } else {
            None
        }
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_pipe_mut(tok).map(|p| p.on_open_ack(event_loop));
    }

    fn send_to(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> Option<mio::Token> {
        if let Some(pipe) = self.pipe.as_mut() {
            if pipe.can_send() {
                pipe.send(event_loop, msg);
                return Some(pipe.token())
            }
        }
        None
    }

    fn on_send_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgSent);
        self.resync_readiness(event_loop, tok);

        clear_timeout(event_loop, timeout);
    }

    fn on_send_done_late(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.resync_readiness(event_loop, tok);
    }

    fn on_send_timeout(&mut self) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.send_notify(SocketNotify::MsgNotSent(err));
    }

    fn recv_from(&mut self, event_loop: &mut EventLoop) -> Option<mio::Token> {
        if let Some(pipe) = self.pipe.as_mut() {
            if pipe.can_recv() {
                pipe.recv(event_loop);
                return Some(pipe.token())
            }
        }
        None
    }

    fn on_recv_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token, msg: Message, timeout: Timeout) {
        self.send_notify(SocketNotify::MsgRecv(msg));
        self.resync_readiness(event_loop, tok);

        clear_timeout(event_loop, timeout);
    }

    fn on_recv_done_late(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.resync_readiness(event_loop, tok);
    }

    fn on_recv_timeout(&mut self) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.send_notify(SocketNotify::MsgNotRecv(err));
    }

    fn resync_readiness(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.get_pipe_mut(tok).map(|p| p.resync_readiness(event_loop));
    }
}
