// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::time::Duration;
use std::io;

use time;

use byteorder::*;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::config::ConfigOption;
use core::endpoint::Pipe;
use core::context::{Context, Schedulable, Event};
use super::priolist::Priolist;
use super::{Timeout, SURVEYOR, RESPONDENT};
use io_error::*;

pub struct Surveyor {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Active(PendingSurvey),
    Receiving(EndpointId, Option<PendingSurvey>, Timeout),
    RecvOnHold(Option<PendingSurvey>, Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: HashMap<EndpointId, Pipe>,
    bc: HashSet<EndpointId>,
    fq: Priolist,
    survey_id_seq: u32,
    is_device_item: bool,
    deadline: Duration
}

struct PendingSurvey {
    id: u32,
    timeout: Timeout
}

/*****************************************************************************/
/*                                                                           */
/* Surveyor                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Surveyor {

    fn apply<F>(&mut self, ctx: &mut Context, transition: F) where F : FnOnce(State, &mut Context, &mut Inner) -> State {
        if let Some(old_state) = self.state.take() {
            #[cfg(debug_assertions)] let old_name = old_state.name();
            let was_send_ready = old_state.is_send_ready(&self.inner);
            let was_recv_ready = old_state.is_recv_ready(&self.inner);
            let new_state = transition(old_state, ctx, &mut self.inner);
            let is_send_ready = new_state.is_send_ready(&self.inner);
            let is_recv_ready = new_state.is_recv_ready(&self.inner);
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

            ctx.check_send_ready_change(was_send_ready, is_send_ready);
            ctx.check_recv_ready_change(was_recv_ready, is_recv_ready);

            #[cfg(debug_assertions)] debug!("[{:?}] switch from {} to {}", ctx, old_name, new_name);
        }
    }

    fn is_send_ready(&self) -> bool {
        if let Some(ref state) = self.state {
            state.is_send_ready(&self.inner)
        } else {
            false
        }
    }

    fn is_recv_ready(&self) -> bool {
        if let Some(ref state) = self.state {
            state.is_recv_ready(&self.inner)
        } else {
            false
        }
    }

}

impl From<Sender<Reply>> for Surveyor {
    fn from(tx: Sender<Reply>) -> Surveyor {
        Surveyor {
            inner: Inner::new(tx),
            state: Some(State::Idle)
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Protocol                                                                  */
/*                                                                           */
/*****************************************************************************/

impl Protocol for Surveyor {
    fn id(&self)      -> u16 { SURVEYOR }
    fn peer_id(&self) -> u16 { RESPONDENT }

    fn add_pipe(&mut self, _: &mut Context, eid: EndpointId, pipe: Pipe) {
        self.inner.add_pipe(eid, pipe)
    }
    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<Pipe> {
        let was_send_ready = self.is_send_ready();
        let was_recv_ready = self.is_recv_ready();
        let pipe = self.inner.remove_pipe(eid);
        let is_send_ready = self.is_send_ready();
        let is_recv_ready = self.is_recv_ready();

        if pipe.is_some() {
            self.apply(ctx, |s, ctx, inner| s.on_pipe_removed(ctx, inner, eid));
        }

        ctx.check_send_ready_change(was_send_ready, is_send_ready);
        ctx.check_recv_ready_change(was_recv_ready, is_recv_ready);

        pipe
    }
    fn send(&mut self, ctx: &mut Context, msg: Message, timeout: Timeout) {
        let raw_msg = self.inner.msg_to_raw_msg(msg);

        self.apply(ctx, |s, ctx, inner| s.send(ctx, inner, Rc::new(raw_msg), timeout))
    }
    fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_send_ack(ctx, inner, eid))
    }
    fn on_send_timeout(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx, inner| s.on_send_timeout(ctx, inner))
    }
    fn on_send_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_send_ready(ctx, inner, eid))
    }
    fn recv(&mut self, ctx: &mut Context, timeout: Timeout) {
        self.apply(ctx, |s, ctx, inner| s.recv(ctx, inner, timeout))
    }
    fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, raw_msg: Message) {
        if let Some((msg, survey_id)) = self.inner.raw_msg_to_msg(raw_msg) {
            self.apply(ctx, |s, ctx, inner| s.on_recv_ack(ctx, inner, eid, msg, survey_id))
        } else {
            self.inner.on_recv_ack_malformed(ctx)
        }
    }
    fn on_recv_timeout(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_timeout(ctx, inner))
    }
    fn on_recv_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_ready(ctx, inner, eid))
    }
    fn set_option(&mut self, opt: ConfigOption) -> io::Result<()> {
        match opt {
            ConfigOption::SurveyDeadline(ivl) => Ok(self.inner.set_survey_deadline(ivl)),
            _ => Err(invalid_input_io_error("option not supported"))
        }
    }
    fn on_timer_tick(&mut self, ctx: &mut Context, task: Schedulable) {
        if let Schedulable::SurveyCancel = task {
            self.apply(ctx, |s, ctx, inner| s.on_survey_timeout(ctx, inner))
        }
    }
    fn on_device_plugged(&mut self, _: &mut Context) {
        self.inner.is_device_item = true;
    }
    fn close(&mut self, ctx: &mut Context) {
        self.inner.close(ctx)
    }
}

/*****************************************************************************/
/*                                                                           */
/* State                                                                     */
/*                                                                           */
/*****************************************************************************/

impl State {

    #[cfg(debug_assertions)]
    fn name(&self) -> &'static str {
        match *self {
            State::Idle           => "Idle",
            State::Active(..)     => "Active",
            State::Receiving(..)  => "Receiving",
            State::RecvOnHold(..) => "RecvOnHold"
        }
    }

    fn on_pipe_removed(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Receiving(id, p, timeout) => {
                if id == eid {
                    State::Idle.recv(ctx, inner, timeout)
                } else {
                    State::Receiving(id, p, timeout)
                }
            },
            any => any
        }
    }

/*****************************************************************************/
/*                                                                           */
/* send                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn send(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Timeout) -> State {
        if let State::Active(p) = self {
            inner.cancel(ctx, p);
        }

        let pending_survey = inner.send(ctx, msg, timeout);

        State::Active(pending_survey)
    }
    fn on_send_ack(self, _: &mut Context, _: &mut Inner, _: EndpointId) -> State {
        self
    }
    fn on_send_timeout(self, _: &mut Context, _: &mut Inner) -> State {
        self
    }
    fn on_send_ready(self, _: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_send_ready(eid);
        self
    }
    fn is_send_ready(&self, inner: &Inner) -> bool {
        inner.is_send_ready()
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn recv(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout) -> State {
        if inner.is_device_item {
            inner.recv(ctx).map_or_else(
                |   | State::RecvOnHold(None, timeout),
                |eid| State::Receiving(eid, None, timeout))
        } else if let State::Active(p) = self {
            State::Idle.recv_reply_for(ctx, inner, timeout, p)
        } else {
            inner.recv_when_inactive(ctx, timeout);

            State::Idle
        }
    }
    fn recv_reply_for(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout, p: PendingSurvey) -> State {
        if let Some(eid) = inner.recv(ctx) {
            State::Receiving(eid, Some(p), timeout)
        } else {
            State::RecvOnHold(Some(p), timeout)
        }
    }
    fn on_recv_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId, msg: Message, survey_id: u32) -> State {
        match self {
            State::Receiving(id, None, timeout) => {
                if id == eid {
                    inner.on_recv_ack(ctx, timeout, msg);
                    State::Idle
                } else {
                    State::Receiving(id, None, timeout)
                }
            },
            State::Receiving(id, Some(p), timeout) => {
                if id == eid {
                    if p.id == survey_id {
                        inner.on_recv_ack(ctx, timeout, msg);
                        State::Active(p)
                    } else {
                        State::Idle.recv_reply_for(ctx, inner, timeout, p)
                    }
                } else {
                    State::Receiving(id, Some(p), timeout)
                }
            },
            any => any
        }
    }
    fn on_recv_timeout(self, _: &mut Context, inner: &mut Inner) -> State {
        inner.on_recv_timeout();

        match self {
            State::Receiving(_, Some(p), _) |
            State::RecvOnHold(Some(p), _)   => State::Active(p),
            _ => State::Idle
        }
    }
    fn on_recv_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_recv_ready(eid);

        match self {
            State::RecvOnHold(None, timeout) => {
                State::Idle.recv(ctx, inner, timeout)
            },
            State::RecvOnHold(Some(p), timeout) => State::Active(p).recv(ctx, inner, timeout),
            any => any
        }
    }
    fn on_survey_timeout(self, _: &mut Context, _: &mut Inner) -> State {
        if let State::Active(_) = self {
            State::Idle
        } else {
            self
        }
    }
    fn is_recv_ready(&self, inner: &Inner) -> bool {
        if inner.is_device_item {
            inner.is_recv_ready()
        } else if let State::Active(..) = *self {
            inner.is_recv_ready()
        } else {
            false
        }
    }

}

/*****************************************************************************/
/*                                                                           */
/* Inner                                                                     */
/*                                                                           */
/*****************************************************************************/

impl Inner {
    fn new(tx: Sender<Reply>) -> Inner {
        Inner {
            reply_tx: tx,
            pipes: HashMap::new(),
            bc: HashSet::new(),
            fq: Priolist::new(),
            survey_id_seq: time::get_time().nsec as u32,
            is_device_item: false,
            deadline: Duration::from_secs(1)
        }
    }
    fn add_pipe(&mut self, eid: EndpointId, pipe: Pipe) {
        self.fq.insert(eid, pipe.get_recv_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.bc.remove(&eid);
        self.fq.remove(&eid);
        self.pipes.remove(&eid)
    }
    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>, timeout: Timeout) -> PendingSurvey {
        for id in self.bc.drain() {
            self.pipes.get_mut(&id).map(|pipe| pipe.send(ctx, msg.clone()));
        }

        let _ = self.reply_tx.send(Reply::Send);
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }

        PendingSurvey {
            id: self.cur_survey_id(),
            timeout: ctx.schedule(Schedulable::SurveyCancel, self.deadline).ok()
        }
    }
    fn on_send_ready(&mut self, eid: EndpointId) {
        self.bc.insert(eid);
    }
    fn is_send_ready(&self) -> bool {
        !self.bc.is_empty()
    }

    fn recv(&mut self, ctx: &mut Context) -> Option<EndpointId> {
        self.fq.pop().map_or(None, |eid| self.recv_from(ctx, eid))
    }
    fn recv_from(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<EndpointId> {
        self.pipes.get_mut(&eid).map_or(None, |pipe| {
            pipe.recv(ctx); 
            Some(eid)
        })
    }
    fn recv_when_inactive(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = other_io_error("Can't recv: no active survey");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_ready(&mut self, eid: EndpointId) {
        self.fq.activate(&eid)
    }
    fn is_recv_ready(&self) -> bool {
        self.fq.peek()
    }
    fn on_recv_ack(&self, ctx: &mut Context, timeout: Timeout, msg: Message) {
        let _ = self.reply_tx.send(Reply::Recv(msg));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_timeout(&self) {
        let error = timedout_io_error("Recv timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn on_recv_ack_malformed(&self, _: &mut Context) {
        let error = invalid_data_io_error("Received response without survey id");
        let _ = self.reply_tx.send(Reply::Err(error));
    }

    fn cancel(&self, ctx: &mut Context, mut pending_survey: PendingSurvey) {
        if let Some(timeout) = pending_survey.timeout.take() {
            ctx.cancel(timeout);
        }
    }

    fn msg_to_raw_msg(&mut self, msg: Message) -> Message {
        if self.is_device_item {
            msg
        } else {
            encode(msg, self.next_survey_id())
        }
    }

    fn raw_msg_to_msg(&self, raw_msg: Message) -> Option<(Message, u32)> {
        if self.is_device_item {
            let cur_survey_id = self.cur_survey_id();
            decode(raw_msg).map(|(msg, _)| (msg, cur_survey_id))
        } else {
            decode(raw_msg).map(|(msg, id)| (msg.without_header(), id))
        }
    }

    fn cur_survey_id(&self) -> u32 {
        self.survey_id_seq | 0x80000000
    }

    fn next_survey_id(&mut self) -> u32 {
        self.survey_id_seq += 1;
        self.survey_id_seq | 0x80000000
    }

    fn set_survey_deadline(&mut self, ivl: Duration) {
        self.deadline = ivl;
    }
    fn close(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
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

fn decode(raw_msg: Message) -> Option<(Message, u32)> {
    if raw_msg.get_body().len() < 4 {
        return None;
    }

    let (mut header, mut payload) = raw_msg.split();
    let body = payload.split_off(4);
    let survey_id = BigEndian::read_u32(&payload);

    if header.is_empty() {
        header = payload;
    } else {
        header.extend_from_slice(&payload);
    }

    Some((Message::from_header_and_body(header, body), survey_id))
}

/*****************************************************************************/
/*                                                                           */
/* tests                                                                     */
/*                                                                           */
/*****************************************************************************/

#[cfg(test)]
mod tests {

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc;

    use byteorder::*;

    use core::{EndpointId, Message};
    use core::socket::{Protocol, Reply};
    use core::context::{Event, Scheduled};
    use core::tests::*;

    use super::*;

    #[test]
    fn when_send_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(1);
        surv.send(&mut ctx, msg, Some(timeout));
        surv.on_send_ack(&mut ctx, eid);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Send => true,
            _ => false
        };
        assert!(is_reply_ok);

        let sensor = ctx_sensor.borrow();
        sensor.assert_one_send_to(eid);
        sensor.assert_one_cancellation(timeout);
    }

    #[test]
    fn when_send_starts_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);
        surv.send(&mut ctx, Message::new(), None);
        surv.on_send_ack(&mut ctx, eid);
        surv.on_send_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
        assert_eq!(Event::CanSend(true), raised_evts[2]);
    }

    #[test]
    fn when_last_send_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(2);
        let pipe = new_test_pipe(eid);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);
        surv.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
    }

    #[test]
    fn when_in_regular_mode_send_will_append_survey_id_to_the_header() {
        let (tx, _) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(4);
        let pipe = new_test_pipe(eid);
        let app_msg = Message::from_body(vec![1u8, 2, 3]);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);
        surv.send(&mut ctx, app_msg, None);

        let sensor = ctx_sensor.borrow();
        let send_calls = sensor.get_send_calls();

        assert_eq!(1, send_calls.len());
        let &(_, ref proto_msg) = &send_calls[0];
        let header = proto_msg.get_header();
        let body = proto_msg.get_body();

        assert_eq!(4, header.len());
        assert_eq!(3, body.len());
        let req_id = BigEndian::read_u32(header);
        let control = req_id & 0x80000000;
        assert_eq!(0x80000000, control);
    }

    #[test]
    fn when_in_raw_mode_send_will_not_append_anything_to_the_header() {
        let (tx, _) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(5);
        let pipe = new_test_pipe(eid);
        let app_msg = Message::from_body(vec![1u8, 2, 3]);

        surv.on_device_plugged(&mut ctx);
        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);
        surv.send(&mut ctx, app_msg, None);

        let sensor = ctx_sensor.borrow();
        let send_calls = sensor.get_send_calls();

        assert_eq!(1, send_calls.len());
        let &(_, ref proto_msg) = &send_calls[0];
        let header = proto_msg.get_header();
        let body = proto_msg.get_body();

        assert_eq!(0, header.len());
        assert_eq!(3, body.len());
    }

    #[test]
    fn when_in_regular_mode_recv_while_idle_will_fail() {
        let (tx, rx) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_recv_ready(&mut ctx, eid);
        surv.recv(&mut ctx, None);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let is_reply_err = match reply {
            Reply::Err(_) => true,
            _ => false
        };
        assert!(is_reply_err);
    }

    #[test]
    fn when_in_regular_mode_recv_while_active_will_drop_msg_with_wrong_request_id() {
        let (tx, rx) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);
        surv.send(&mut ctx, Message::new(), None);
        surv.on_send_ack(&mut ctx, eid);
        let _ = rx.try_recv().expect("facade should have been sent a reply !");

        let bad_survey_id = (surv.inner.survey_id_seq - 1) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], bad_survey_id);

        let msg = Message::from_body(body);
        surv.on_recv_ready(&mut ctx, eid);
        surv.recv(&mut ctx, None);
        surv.on_recv_ack(&mut ctx, eid, msg);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn when_in_regular_mode_recv_while_active_will_accept_msg_with_right_request_id() {
        let (tx, rx) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);
        surv.send(&mut ctx, Message::new(), None);
        surv.on_send_ack(&mut ctx, eid);
        let _ = rx.try_recv().expect("facade should have been sent a reply !");

        let good_survey_id = (surv.inner.survey_id_seq) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], good_survey_id);

        let msg = Message::from_body(body);
        surv.on_recv_ready(&mut ctx, eid);
        surv.recv(&mut ctx, None);
        surv.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Recv(_) => true,
            _ => false
        };
        assert!(is_reply_ok);
    }

    #[test]
    fn when_in_regular_mode_recv_removes_the_survey_id_from_the_body() {
        let (tx, rx) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_send_ready(&mut ctx, eid);
        surv.send(&mut ctx, Message::new(), None);
        surv.on_send_ack(&mut ctx, eid);
        let _ = rx.try_recv().expect("facade should have been sent a reply !");

        let good_survey_id = (surv.inner.survey_id_seq) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], good_survey_id);

        let msg = Message::from_body(body);
        surv.on_recv_ready(&mut ctx, eid);
        surv.recv(&mut ctx, None);
        surv.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.unwrap();
        assert_eq!(0, app_msg.get_header().len());
        assert_eq!(3, app_msg.get_body().len());
    }

    #[test]
    fn when_in_raw_mode_recv_moves_the_survey_id_from_the_body_to_the_header() {
        let (tx, rx) = mpsc::channel();
        let mut surv = Surveyor::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        let any_survey_id = 666 | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], any_survey_id);

        let msg = Message::from_body(body);
        surv.on_device_plugged(&mut ctx);
        surv.add_pipe(&mut ctx, eid, pipe);
        surv.on_recv_ready(&mut ctx, eid);
        surv.recv(&mut ctx, None);
        surv.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.expect("facade should have been sent a Recv reply !");
        assert_eq!(4, app_msg.get_header().len());
        assert_eq!(3, app_msg.get_body().len());
    }
}