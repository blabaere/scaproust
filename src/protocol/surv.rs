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

use time as ext_time;

use std::time;

use byteorder::*;

use super::{ Protocol, Timeout };
use super::clear_timeout;
use super::priolist::*;
use pipe::*;
use global::*;
use event_loop_msg::{ SocketNotify, EventLoopTimeout, SocketOption };
use EventLoop;
use Message;

pub struct Surv {
    id: SocketId,
    body: Body,
    state: Option<State>
}

struct Body {
    id: SocketId,
    notify_sender: Rc<Sender<SocketNotify>>,
    pipes: HashMap<mio::Token, Pipe>,
    dist: HashSet<mio::Token>,
    fq: PrioList,
    survey_id_seq: u32,
    deadline_ms: u64
}

struct PendingSurvey {
    timeout: Timeout
}

enum State {
    Idle,
    Active(PendingSurvey),
    Receiving(PendingSurvey, Timeout),
    RecvOnHold(PendingSurvey, Timeout)
}

impl Surv {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Surv {
        let body = Body {
            id: socket_id,
            notify_sender: notify_tx,
            pipes: HashMap::new(),
            dist: HashSet::new(),
            fq: PrioList::new(),
            survey_id_seq: ext_time::get_time().nsec as u32,
            deadline_ms: 1_000
        };

        Surv {
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

impl Protocol for Surv {
    fn id(&self) -> u16 {
        SocketType::Surveyor.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Respondent.id()
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
        let survey_id = self.body.next_survey_id();
        let msg = encode(msg, survey_id);

        self.apply(|s, body| s.send(body, event_loop, Rc::new(msg), timeout));
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
        if let Some((msg, survey_id)) = decode(raw_msg, tok) {
            self.apply(|s, body| s.on_recv_by_pipe(body, event_loop, tok, msg, survey_id));
        }
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_recv_timeout(body, event_loop));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.apply(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn on_survey_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_survey_timeout(body, event_loop));
    }

    fn set_option(&mut self, _: &mut EventLoop, option: SocketOption) -> io::Result<()> {
        match option {
            SocketOption::SurveyDeadline(timeout) => self.body.set_deadline(timeout),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
        }
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle            => "Idle",
            State::Active(_)       => "WaitingVotes",
            State::Receiving(_,_)  => "Receiving",
            State::RecvOnHold(_,_) => "RecvOnHold"
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, body: &mut Body, tok: mio::Token) -> State {
        match self {
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

    fn open_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.open_pipe(event_loop, tok);

        self
    }

    fn on_pipe_opened(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.on_pipe_opened(event_loop, tok);

        self
    }

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, _: Option<mio::Timeout>) -> State {
        if let State::Active(p) = self {
            clear_timeout(event_loop, p.timeout);
        }

        State::Active(body.send(event_loop, msg))
    }

    fn on_send_by_pipe(self, _: &mut Body, _: &mut EventLoop, _: mio::Token) -> State {
        self
    }

    fn on_send_timeout(self, _: &mut Body, _: &mut EventLoop) -> State {
        self
    }

    fn on_survey_timeout(self, _: &mut Body, _: &mut EventLoop) -> State {
        State::Idle
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        if let State::Active(p) = self {
            try_recv(body, event_loop, timeout, p)
        } else {
            let err = other_io_error("Can't recv: currently no pending survey");
            body.send_notify(SocketNotify::MsgNotRecv(err));
            clear_timeout(event_loop, timeout);

            self
        }
    }

    fn on_recv_by_pipe(self, body: &mut Body, event_loop: &mut EventLoop, _: mio::Token, msg: Message, survey_id: u32) -> State {
        if let State::Receiving(p, timeout) = self {
            if survey_id == body.cur_survey_id() {
                body.on_recv_by_pipe(event_loop, msg, timeout);

                State::Active(p)
            } else {
                try_recv(body, event_loop, timeout, p)
            }
        } else {
            State::Idle
        }
    }

    fn on_recv_timeout(self, body: &mut Body, event_loop: &mut EventLoop) -> State {
        body.on_recv_timeout(event_loop);

        match self {
            State::Receiving(p, _)  => State::Active(p),
            State::RecvOnHold(p, _) => State::Active(p),
            _                       => State::Idle
        }
    }

    fn ready(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) -> State {
        body.ready(event_loop, tok, events);

        match self {
            State::RecvOnHold(p, t) => try_recv(body, event_loop, t, p),
            other @ _               => other
        }
    }
}

fn try_recv(body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>, p: PendingSurvey) -> State {
    if body.recv(event_loop) {
        State::Receiving(p, timeout)
    } else {
        State::RecvOnHold(p, timeout)
    }
}

impl Body {

    fn set_deadline(&mut self, deadline: time::Duration) -> io::Result<()> {
        let deadline_ms = deadline.to_millis();

        if deadline_ms == 0u64 {
            Err(io::Error::new(io::ErrorKind::InvalidData, "survey deadline cannot be zero"))
        } else {
            self.deadline_ms = deadline_ms;
            Ok(())
        }
    }

    fn schedule_deadline(&mut self, event_loop: &mut EventLoop) -> Timeout {
        let cmd = EventLoopTimeout::CancelSurvey(self.id);
        let ivl = self.deadline_ms;

        event_loop.timeout(cmd, time::Duration::from_millis(ivl)).
            map(|t| Some(t)).
            unwrap_or_else(|_| None)
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

    fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>,) -> PendingSurvey {
        self.broadcast(event_loop, msg);
        self.send_notify(SocketNotify::MsgSent);

        PendingSurvey {
            timeout: self.schedule_deadline(event_loop)
        }
    }

    fn broadcast(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) {
        for tok in self.dist.iter() {
            let msg = msg.clone();

            self.pipes.get_mut(tok).map(|p| p.send_nb(event_loop, msg));
        }
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

    fn cur_survey_id(&self) -> u32 {
        self.survey_id_seq | 0x80000000
    }

    fn next_survey_id(&mut self) -> u32 {
        self.survey_id_seq += 1;
        self.survey_id_seq | 0x80000000
    }
}

fn encode(msg: Message, survey_id: u32) -> Message {
    let mut raw_msg = msg;
    let mut survey_id_bytes: [u8; 4] = [0; 4];

    BigEndian::write_u32(&mut survey_id_bytes[0..4], survey_id);

    raw_msg.header.reserve(4);
    raw_msg.header.extend_from_slice(&survey_id_bytes[0..4]);
    raw_msg
}

fn decode(raw_msg: Message, _: mio::Token) -> Option<(Message, u32)> {
    if raw_msg.get_body().len() < 4 {
        return None;
    }

    let (mut header, mut payload) = raw_msg.explode();
    let body = payload.split_off(4);
    let survey_id = BigEndian::read_u32(&payload);

    if header.len() == 0 {
        header = payload;
    } else {
        header.extend_from_slice(&payload);
    }

    Some((Message::with_header_and_body(header, body), survey_id))
}
