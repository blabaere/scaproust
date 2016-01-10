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

use time;

use byteorder::*;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use super::priolist::*;
use pipe::*;
use global::*;
use event_loop_msg::{ SocketNotify, EventLoopTimeout };
use EventLoop;
use Message;

pub struct Req {
    id: SocketId,
    body: Body,
    state: Option<State>
}

struct Body {
    id: SocketId,
    notify_sender: Rc<Sender<SocketNotify>>,
    pipes: HashMap<mio::Token, Pipe>,
    lb: PrioList,
    req_id_seq: u32,
    resend_interval: u64
}

enum State {
    Idle,
    Sending(Rc<Message>, Timeout),
    SendOnHold(Rc<Message>, Timeout),
    WaitingReply(PendingRequest),
    Receiving(PendingRequest, Timeout),
    RecvOnHold(PendingRequest, Timeout)
}

struct PendingRequest {
    peer: mio::Token,
    req: Rc<Message>,
    timeout: Timeout
}

impl Req {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Req {
        let body = Body {
            id: socket_id,
            notify_sender: notify_tx,
            pipes: HashMap::new(),
            lb: PrioList::new(),
            req_id_seq: time::get_time().nsec as u32,
            resend_interval: 60_000
        };

        Req {
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

impl Protocol for Req {
    fn id(&self) -> u16 {
        SocketType::Req.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Rep.id()
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

    fn register_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.on_state_transition(|s, body| s.register_pipe(body, event_loop, tok));
    }

    fn on_pipe_register(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.on_state_transition(|s, body| s.on_pipe_register(body, event_loop, tok));
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout: Timeout) {
        let req_id = self.body.next_req_id();
        let msg = encode(msg, req_id);

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

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token, raw_msg: Message) {
        if let Some((msg, req_id)) = decode(raw_msg, tok) {
            if req_id == self.body.cur_req_id() {
                self.on_state_transition(|s, body| s.on_recv_by_pipe(body, event_loop, tok, msg));
            }
        }
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s, body| s.on_recv_timeout(body, event_loop));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.on_state_transition(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn resend(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s, body| s.resend(body, event_loop));
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle            => "Idle",
            State::Sending(_, _)   => "Sending",
            State::SendOnHold(_,_) => "SendOnHold",
            State::WaitingReply(_) => "WaitingReply",
            State::Receiving(_,_)  => "Receiving",
            State::RecvOnHold(_,_) => "RecvOnHold"
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, body: &mut Body, tok: mio::Token) -> State {
        match self {
            State::Sending(msg, t) => {
                if body.is_active_pipe(tok) {
                    State::SendOnHold(msg, t)
                } else {
                    State::Sending(msg, t)
                }
            },
            State::Receiving(p, t) => {
                if body.is_active_pipe(tok) {
                    State::RecvOnHold(p, t)
                } else {
                    State::Receiving(p, t)
                }
            },
            other @ _ => other
        }
    }

    fn register_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.register_pipe(event_loop, tok);

        self
    }

    fn on_pipe_register(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.on_pipe_register(event_loop, tok);

        match self {
            State::SendOnHold(msg, t) => State::Idle.send(body, event_loop, msg, t),
            State::RecvOnHold(_, t)   => State::Idle.recv(body, event_loop, t),
            other @ _                 => other
        }
    }

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Option<mio::Timeout>) -> State {
        if let State::WaitingReply(p) = self {
            clear_timeout(event_loop, p.timeout);
        }

        if let Some(pipe) = body.get_active_pipe() {
            pipe.send(event_loop, msg.clone());

            return State::Sending(msg, timeout);
        } else {
            return State::SendOnHold(msg, timeout);
        }
    }

    fn on_send_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        if let State::Sending(msg, timeout) = self {
            let resend_timeout = body.schedule_resend(event_loop);
            let pending_request = PendingRequest {
                peer: tok,
                req: msg,
                timeout: resend_timeout
            };

            body.send_notify(SocketNotify::MsgSent);

            clear_timeout(event_loop, timeout);

            State::WaitingReply(pending_request)
        } else {
            State::Idle
        }
    }

    fn on_send_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        body.send_notify(SocketNotify::MsgNotSent(err));
        body.get_active_pipe().map(|p| p.cancel_send(event_loop));
        body.advance_pipe();

        State::Idle
    }

    fn resend(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        if let State::WaitingReply(p) = self {
            let resend_timeout = body.schedule_resend(event_loop);
            let pending_request = PendingRequest {
                peer: p.peer,
                req: p.req,
                timeout: resend_timeout
            };

            
            State::WaitingReply(pending_request)
        } else {
            // can't send the request again while receiving because 
            // a pipe has one state and cannot both recv and send a the same time
            self
        }
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        if let State::WaitingReply(p) = self {
            if let Some(pipe) = body.get_pipe(p.peer) {
                pipe.recv(event_loop);

                return State::Receiving(p, timeout);
            } else {
                return State::RecvOnHold(p, timeout);
            }
        } else {
            let err = other_io_error("Can't recv: currently no pending request");
            body.send_notify(SocketNotify::MsgNotRecv(err));
            clear_timeout(event_loop, timeout);

            self
        }
    }

    fn on_recv_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, _: mio::Token, msg: Message) -> State {
        if let State::Receiving(p, timeout) = self {
            body.send_notify(SocketNotify::MsgRecv(msg));
            body.advance_pipe();
            clear_timeout(event_loop, timeout);
            clear_timeout(event_loop, p.timeout);
        }

        State::Idle
    }

    fn on_recv_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        body.send_notify(SocketNotify::MsgNotRecv(err));
        body.get_active_pipe().map(|p| p.cancel_recv(event_loop));
        body.advance_pipe();

        match self {
            State::Receiving(p, _)  => clear_timeout(event_loop, p.timeout),
            State::RecvOnHold(p, _) => clear_timeout(event_loop, p.timeout),
            _                       => {}
        };

        State::Idle
    }

    fn ready(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) -> State {
        body.get_pipe(tok).map(|p| p.ready(event_loop, events));

        self
    }
}

impl Body {

    fn schedule_resend(&mut self, event_loop: &mut EventLoop) -> Timeout {
        if self.resend_interval > 0u64 {
            let cmd = EventLoopTimeout::Resend(self.id);
            let ivl = self.resend_interval;

            event_loop.timeout_ms(cmd, ivl).
                map(|t| Some(t)).
                unwrap_or_else(|_| None)
        } else {
            None
        }
    }

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
    }

    fn get_active_pipe<'a>(&'a mut self) -> Option<&'a mut Pipe> {
        match self.lb.get() {
            Some(tok) => self.pipes.get_mut(&tok),
            None      => None
        }
    }

    fn is_active_pipe(&self, tok: mio::Token) -> bool {
        self.lb.get() == Some(tok)
    }

    fn advance_pipe(&mut self) {
        self.lb.advance();
    }

    fn get_pipe<'a>(&'a mut self, tok: mio::Token) -> Option<&'a mut Pipe> {
        self.pipes.get_mut(&tok)
    }

    fn cur_req_id(&self) -> u32 {
        self.req_id_seq | 0x80000000
    }

    fn next_req_id(&mut self) -> u32 {
        self.req_id_seq += 1;
        self.req_id_seq | 0x80000000
    }
}

fn encode(msg: Message, req_id: u32) -> Message {
    let mut raw_msg = msg;
    let mut req_id_bytes: [u8; 4] = [0; 4];

    BigEndian::write_u32(&mut req_id_bytes[0..4], req_id);

    raw_msg.header.reserve(4);
    raw_msg.header.extend_from_slice(&req_id_bytes[0..4]);
    raw_msg
}

fn decode(raw_msg: Message, _: mio::Token) -> Option<(Message, u32)> {
    if raw_msg.get_body().len() < 4 {
        return None;
    }

    let (mut header, mut payload) = raw_msg.explode();
    let body = payload.split_off(4);
    let req_id = BigEndian::read_u32(&payload);

    if header.len() == 0 {
        header = payload;
    } else {
        header.extend_from_slice(&payload);
    }

    Some((Message::with_header_and_body(header, body), req_id))
}
