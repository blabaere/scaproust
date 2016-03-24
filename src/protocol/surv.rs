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

use time as ext_time;

use std::time;

use byteorder::*;

use protocol::Protocol;
use protocol::policy::*;
use pipe::Pipe;
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
    fq: PrioList,
    survey_id_seq: u32,
    deadline_ms: u64,
    is_device_item: bool
}

struct PendingSurvey {
    timeout: Timeout
}

enum State {
    Idle,
    Active(PendingSurvey),
    Receiving(mio::Token, PendingSurvey, Timeout),
    RecvOnHold(PendingSurvey, Timeout)
}

impl Surv {
    pub fn new(socket_id: SocketId, notify_tx: Rc<Sender<SocketNotify>>) -> Surv {
        let body = Body {
            id: socket_id,
            notify_sender: notify_tx,
            pipes: HashMap::new(),
            fq: PrioList::new(),
            survey_id_seq: ext_time::get_time().nsec as u32,
            deadline_ms: 1_000,
            is_device_item: false
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
    fn get_type(&self) -> SocketType {
        SocketType::Surveyor
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
        //let survey_id = self.body.next_survey_id();
        //let msg = encode(msg, survey_id);
        let raw_msg = self.body.msg_to_raw_msg(msg);

        self.apply(|s, body| s.send(body, event_loop, Rc::new(raw_msg), timeout));
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

    fn on_recv_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token, raw_msg: Message) {
        if let Some((msg, survey_id)) = self.body.raw_msg_to_msg(raw_msg, tok) {
            self.apply(|s, body| s.on_recv_done(body, event_loop, tok, msg, survey_id));
        } else {
            // TODO notify a recv failure, or restart recv
        }
    }

    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
        self.apply(|s, body| s.on_recv_timeout(body));
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        self.apply(|s, body| s.ready(body, event_loop, tok, events));
    }

    fn on_survey_timeout(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s, body| s.on_survey_timeout(body, event_loop));
    }

    fn can_recv(&self) -> bool { 
        self.body.can_recv()
    }

    fn set_option(&mut self, _: &mut EventLoop, option: SocketOption) -> io::Result<()> {
        match option {
            SocketOption::SurveyDeadline(timeout) => self.body.set_deadline(timeout),
            _ => Err(invalid_input_io_error("option not supported by protocol"))
        }
    }

    fn set_device_item(&mut self, value: bool) -> io::Result<()> {
        self.body.set_device_item(value)
    }

    fn destroy(&mut self, event_loop: &mut EventLoop) {
        self.body.destroy_pipes(event_loop);
    }
}

impl State {
    fn name(&self) -> &'static str {
        match *self {
            State::Idle             => "Idle",
            State::Active(_)        => "WaitingVotes",
            State::Receiving(_,_,_) => "Receiving",
            State::RecvOnHold(_,_)  => "RecvOnHold"
        }
    }

    fn on_pipe_added(self, _: &mut Body, _: mio::Token) -> State {
        self
    }

    fn on_pipe_removed(self, _: &mut Body, tok: mio::Token) -> State {
        match self {
            State::Receiving(token, pending, timeout) => {
                if token == tok {
                    State::RecvOnHold(pending, timeout)
                } else {
                    State::Receiving(token, pending, timeout)
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

    fn send(self, body: &mut Body, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Option<mio::Timeout>) -> State {
        if let State::Active(p) = self {
            clear_timeout(event_loop, p.timeout);
        }

        State::Active(body.send(event_loop, msg, timeout))
    }

    fn on_send_done(self, _: &mut Body, _: &mut EventLoop, _: mio::Token) -> State {
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

    fn on_recv_done(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, msg: Message, survey_id: u32) -> State {
        if let State::Receiving(token, p, timeout) = self {
            if token == tok {
                if survey_id == body.cur_survey_id() {
                    body.on_recv_done(event_loop, msg, timeout);

                     State::Active(p)
                } else {
                    try_recv(body, event_loop, timeout, p)
                }
            } else {
                body.on_recv_done_late(event_loop, tok);
                State::Receiving(token, p, timeout)
            }
        } else {
            body.on_recv_done_late(event_loop, tok);
            State::Idle
        }
    }

    fn on_recv_timeout(self, body: &mut Body) -> State {
        body.on_recv_timeout();

        match self {
            State::Receiving(_,p, _) |
            State::RecvOnHold(p, _)   => State::Active(p),
            other                     => other
        }
    }

    fn ready(self, body: &mut Body, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) -> State {
        body.ready(event_loop, tok, events);

        match self {
            State::RecvOnHold(p, t) => try_recv(body, event_loop, t, p),
            other                   => other
        }
    }
}

fn try_recv(body: &mut Body, event_loop: &mut EventLoop, timeout: Option<mio::Timeout>, p: PendingSurvey) -> State {
    if let Some(tok) = body.recv_from(event_loop) {
        State::Receiving(tok, p, timeout)
    } else {
        State::RecvOnHold(p, timeout)
    }
}

impl Body {

    fn set_device_item(&mut self, value: bool) -> io::Result<()> {
        self.is_device_item = value;
        Ok(())
    }

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
            map(Some).
            unwrap_or_else(|_| None)
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>, timeout: Timeout) -> PendingSurvey {
        self.send_all(event_loop, msg, timeout);

        PendingSurvey {
            timeout: self.schedule_deadline(event_loop)
        }
    }

    fn raw_msg_to_msg(&self, raw_msg: Message, tok: mio::Token) -> Option<(Message, u32)> {
        if self.is_device_item {
            Some((raw_msg, self.cur_survey_id()))
        } else {
            decode(raw_msg, tok)
        }
    }

    fn msg_to_raw_msg(&mut self, msg: Message) -> Message {
        if self.is_device_item {
            msg
        } else {
            encode(msg, self.next_survey_id())
        }
    }

    fn cur_survey_id(&self) -> u32 {
        self.survey_id_seq | 0x80000000
    }

    fn next_survey_id(&mut self) -> u32 {
        self.survey_id_seq += 1;
        self.survey_id_seq | 0x80000000
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

impl WithBroadcast for Body {
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

    if header.is_empty() {
        header = payload;
    } else {
        header.extend_from_slice(&payload);
    }

    Some((Message::with_header_and_body(header, body), survey_id))
}
