// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::{ HashMap, HashSet };
use std::sync::mpsc::Sender;
use std::io;

use mio;

use time as ext_time;

use std::time;

use byteorder::*;

use super::Protocol;
use super::clear_timeout;
use super::priolist::*;
use pipe::*;
use global::*;
use event_loop_msg::{ SocketNotify, EventLoopTimeout, SocketOption };
use EventLoop;
use Message;

type Timeout = Option<mio::Timeout>;

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
        // TODO set deadline timer
        //self.reset_survey_deadline_timeout(event_loop);
        let survey_id = self.body.next_survey_id();
        let msg = encode(msg, survey_id);

        self.on_state_transition(|s, body| s.send(body, event_loop, Rc::new(msg), timeout));
    }

    fn on_send_by_pipe(&mut self, _: &mut EventLoop, _: mio::Token) {
    }

    fn on_send_timeout(&mut self, _: &mut EventLoop) {
    }

    fn recv(&mut self, event_loop: &mut EventLoop, timeout: Timeout) {
        self.on_state_transition(|s, body| s.recv(body, event_loop, timeout));
    }

    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token, raw_msg: Message) {
        if let Some((msg, survey_id)) = decode(raw_msg, tok) {
            if survey_id == self.body.cur_survey_id() {
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

    fn set_option(&mut self, _: &mut EventLoop, option: SocketOption) -> io::Result<()> {
        match option {
            SocketOption::SurveyDeadline(timeout) => self.body.set_deadline(timeout),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
        }
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {
        // TODO do something about that
        //self.codec.clear_pending_survey();
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

    fn register_pipe(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.register_pipe(event_loop, tok);

        self
    }

    fn on_pipe_register(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token) -> State {
        body.on_pipe_register(event_loop, tok);

        match self {
            State::RecvOnHold(_, t)   => State::Idle.recv(body, event_loop, t),
            other @ _                 => other
        }
    }

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Option<mio::Timeout>) -> State {
        if let State::Active(p) = self {
            clear_timeout(event_loop, p.timeout);
        }

        for (tok, mut pipe) in body.pipes.iter_mut() {
            if body.dist.contains(tok) {
                pipe.send_nb(event_loop, msg.clone());
            }
        }

        body.send_notify(SocketNotify::MsgSent);

        let deadline = body.schedule_deadline(event_loop);
        let pending_survey = PendingSurvey {
            timeout: deadline
        };

        State::Active(pending_survey)
    }

    fn recv(self, body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>) -> State {
        if let State::Active(p) = self {
            if let Some(pipe) = body.get_active_pipe() {
                pipe.recv(event_loop);

                return State::Receiving(p, timeout);
            } else {
                return State::RecvOnHold(p, timeout);
            }
        } else {
            let err = other_io_error("Can't recv: currently no pending survey");
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

        event_loop.timeout_ms(cmd, ivl).
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

    fn register_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.fq.insert(tok, 8);
        self.pipes.get_mut(&tok).map(|p| p.register(event_loop));
    }

    fn on_pipe_register(&mut self, event_loop: &mut EventLoop, tok: mio::Token) {
        self.dist.insert(tok);
        self.fq.activate(tok);
        self.pipes.get_mut(&tok).map(|p| p.on_register(event_loop));
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
