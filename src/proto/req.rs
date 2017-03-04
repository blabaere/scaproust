// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;
use std::time::Duration;

use time;

use byteorder::*;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::config::ConfigOption;
use core::endpoint::Pipe;
use core::context::{Context, Schedulable};
use super::priolist::Priolist;
use super::pipes::PipeCollection;
use super::{Timeout, REQ, REP};
use super::policy::{load_balancing, fair_queue};
use io_error::*;

pub struct Req {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Sending(EndpointId, Rc<Message>, Timeout, bool),
    SendOnHold(Rc<Message>, Timeout, bool),
    Active(EndpointId, PendingRequest),
    Receiving(EndpointId, Option<PendingRequest>, Timeout),
    RecvOnHold(Option<EndpointId>, Option<PendingRequest>, Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: PipeCollection,
    lb: Priolist,
    fq: Priolist,
    rv: HashSet<EndpointId>,
    req_id_seq: u32,
    is_device_item: bool,
    resend_ivl: Duration
}

struct PendingRequest {
    req: Rc<Message>,
    retry_timeout: Timeout
}

/*****************************************************************************/
/*                                                                           */
/* Req                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Req {

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

}

impl From<Sender<Reply>> for Req {
    fn from(tx: Sender<Reply>) -> Req {
        Req {
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

impl Protocol for Req {
    fn id(&self)      -> u16 { REQ }
    fn peer_id(&self) -> u16 { REP }

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

        self.apply(ctx, |s, ctx, inner| s.send(ctx, inner, Rc::new(raw_msg), timeout, false))
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
        if let Some((msg, req_id)) = self.inner.raw_msg_to_msg(raw_msg) {
            self.apply(ctx, |s, ctx, inner| s.on_recv_ack(ctx, inner, eid, msg, req_id))
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
            ConfigOption::ReqResendIvl(ivl) => Ok(self.inner.set_resend_ivl(ivl)),
            _ => Err(invalid_input_io_error("option not supported"))
        }
    }
    fn on_timer_tick(&mut self, ctx: &mut Context, task: Schedulable) {
        if let Schedulable::ReqResend = task {
            self.apply(ctx, |s, ctx, inner| s.on_retry_timeout(ctx, inner))
        }
    }
    fn on_device_plugged(&mut self, _: &mut Context) {
        self.inner.is_device_item = true;
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
            State::Idle                    => "Idle",
            State::Sending(_, _, _, false) => "Sending",
            State::Sending(_, _, _, true)  => "Resending",
            State::SendOnHold(_, _, _)     => "SendOnHold",
            State::Active(..)               => "Active",
            State::Receiving(..)         => "Receiving",
            State::RecvOnHold(..)        => "RecvOnHold"
        }
    }

    fn on_pipe_removed(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Sending(id, msg, timeout, retry) => {
                if id == eid {
                    State::Idle.send(ctx, inner, msg, timeout, retry)
                } else {
                    State::Sending(id, msg, timeout, retry)
                }
            },
            State::Receiving(id, p, timeout) => {
                if eid == id {
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

    fn send(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Timeout, retry: bool) -> State {
        if let State::Active(_, p) = self {
            inner.cancel(ctx, p);
        }
        if let Some(eid) = inner.send(ctx, msg.clone()) {
            State::Sending(eid, msg, timeout, retry)
        } else {
            State::SendOnHold(msg, timeout, retry)
        }
    }
    fn on_send_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Sending(id, msg, timeout, retry) => {
                if id != eid {
                    return State::Sending(id, msg, timeout, retry);
                }

                let retry_timeout = inner.on_send_ack(ctx, timeout, retry);

                if inner.is_device_item {
                    State::Idle
                } else {
                    State::Active(eid, PendingRequest {
                        req: msg,
                        retry_timeout: retry_timeout
                    })
                }
            },
            any => any
        }
    }
    fn on_send_timeout(self, _: &mut Context, inner: &mut Inner) -> State {
        inner.on_send_timeout();

        State::Idle
    }
    fn on_send_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_send_ready(eid);

        match self {
            State::SendOnHold(msg, timeout, retry) => State::Idle.send(ctx, inner, msg, timeout, retry),
            any => any
        }
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
                |   | State::RecvOnHold(None, None, timeout),
                |eid| State::Receiving(eid, None, timeout))
        } else if let State::Active(eid, p) = self {
            State::Idle.recv_reply_for(ctx, inner, timeout, eid, p)
        } else {
            inner.recv_when_inactive(ctx, timeout);

            State::Idle
        }
    }
    fn recv_reply_for(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout, eid: EndpointId, p: PendingRequest) -> State {
        if inner.recv_reply_from(ctx, eid) {
            State::Receiving(eid, Some(p), timeout)
        } else {
            State::RecvOnHold(Some(eid), Some(p), timeout)
        }
    }
    fn on_recv_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId, msg: Message, req_id: u32) -> State {
        match self {
            State::Receiving(id, None, timeout) => {
                if eid == id {
                    inner.on_recv_ack(ctx, timeout, msg, None);
                    State::Idle
                } else {
                    State::Receiving(id, None, timeout)
                }
            },
            State::Receiving(id, Some(p), timeout) => {
                if eid == id {
                    if inner.cur_req_id() == req_id {
                        inner.on_recv_ack(ctx, timeout, msg, p.retry_timeout);
                        State::Idle
                    } else {
                        State::Idle.recv_reply_for(ctx, inner, timeout, id, p)
                    }
                } else {
                    State::Receiving(id, Some(p), timeout)
                }
            },
            any => any
        }
    }
    fn on_recv_timeout(self, ctx: &mut Context, inner: &mut Inner) -> State {
        match self {
            State::Receiving(_, None, _) |
            State::RecvOnHold(_, None, _) => inner.on_recv_timeout(ctx, None),
            State::Receiving(_, Some(p), _) |
            State::RecvOnHold(_, Some(p), _) => inner.on_recv_timeout(ctx, p.retry_timeout),
            _ => {}
        }

        State::Idle
    }
    fn on_recv_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_recv_ready(eid);
        match self {
            State::RecvOnHold(None, None, timeout) => {
                State::Idle.recv(ctx, inner, timeout)
            },
            State::RecvOnHold(Some(id), Some(p), timeout) => {
                if eid == id {
                    State::Active(id, p).recv(ctx, inner, timeout)
                } else {
                    State::RecvOnHold(Some(id), Some(p), timeout)
                }
            },
            any => any
        }
    }
    fn on_retry_timeout(self, ctx: &mut Context, inner: &mut Inner) -> State {
        if let State::Active(_, p) = self {
            State::Idle.send(ctx, inner, p.req, None, true)
        } else {
            self
        }
    }
    fn is_recv_ready(&self, inner: &Inner) -> bool {
        if inner.is_device_item {
            inner.is_recv_ready()
        } else if let State::Active(ref eid, _) = *self {
            inner.is_recv_ready_from(eid)
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
            pipes: PipeCollection::new(),
            lb: Priolist::new(),
            fq: Priolist::new(),
            rv: HashSet::new(),
            req_id_seq: time::get_time().nsec as u32,
            is_device_item: false,
            resend_ivl: Duration::from_secs(60)
        }
    }
    fn add_pipe(&mut self, eid: EndpointId, pipe: Pipe) {
        self.lb.insert(eid, pipe.get_send_priority());
        self.fq.insert(eid, pipe.get_recv_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.lb.remove(&eid);
        self.fq.remove(&eid);
        self.rv.remove(&eid);
        self.pipes.remove(&eid)
    }
    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>) -> Option<EndpointId> {
        load_balancing::send(&mut self.lb, &mut self.pipes, ctx, msg)
    }
    fn on_send_ready(&mut self, eid: EndpointId) {
        self.lb.activate(&eid)
    }
    fn on_send_ack(&self, ctx: &mut Context, timeout: Timeout, retry: bool) -> Timeout {
        if !retry {
            let _ = self.reply_tx.send(Reply::Send);
        }
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
        if self.is_device_item {
            None
        } else {
            ctx.schedule(Schedulable::ReqResend, self.resend_ivl).ok()
        }
    }
    fn on_send_timeout(&self) {
        let error = timedout_io_error("Send timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn is_send_ready(&self) -> bool {
        self.lb.peek()
    }
    fn cancel(&self, ctx: &mut Context, p: PendingRequest) {
        if let Some(sched) = p.retry_timeout {
            ctx.cancel(sched);
        }
    }

    fn recv(&mut self, ctx: &mut Context) -> Option<EndpointId> {
        fair_queue::recv(&mut self.fq, &mut self.pipes, ctx)
    }
    fn recv_reply_from(&mut self, ctx: &mut Context, eid: EndpointId) -> bool {
        self.rv.remove(&eid);
        self.pipes.get_mut(&eid).map(|pipe| pipe.recv(ctx)).is_some()
    }
    fn recv_when_inactive(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = other_io_error("Can't recv: no active request");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_ack(&self, ctx: &mut Context, timeout: Timeout, msg: Message, retry_timeout: Timeout) {
        let _ = self.reply_tx.send(Reply::Recv(msg));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
        if let Some(sched) = retry_timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_timeout(&self, ctx: &mut Context, retry_timeout: Timeout) {
        let error = timedout_io_error("Recv timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = retry_timeout {
            ctx.cancel(sched);
        }
    }
    fn on_recv_ack_malformed(&self, _: &mut Context) {
        let error = invalid_data_io_error("Received reply without req id");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn on_recv_ready(&mut self, eid: EndpointId) {
        self.fq.activate(&eid);
        self.rv.insert(eid);
    }
    fn is_recv_ready_from(&self, eid: &EndpointId) -> bool {
        self.rv.contains(eid)
    }
    fn is_recv_ready(&self) -> bool {
        self.fq.peek()
    }

    fn msg_to_raw_msg(&mut self, msg: Message) -> Message {
        if self.is_device_item {
            msg
        } else {
            encode(msg, self.next_req_id())
        }
    }

    fn raw_msg_to_msg(&self, raw_msg: Message) -> Option<(Message, u32)> {
        if self.is_device_item {
            let cur_req_id = self.cur_req_id();
            decode(raw_msg).map(|(msg, _)| (msg, cur_req_id))
        } else {
            decode(raw_msg)
        }
    }

    fn cur_req_id(&self) -> u32 {
        self.req_id_seq | 0x80000000
    }

    fn next_req_id(&mut self) -> u32 {
        self.req_id_seq += 1;
        self.req_id_seq | 0x80000000
    }
    fn set_resend_ivl(&mut self, ivl: Duration) {
        self.resend_ivl = ivl;
    }
    fn close(&mut self, ctx: &mut Context) {
        self.pipes.close_all(ctx)
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

fn decode(raw_msg: Message) -> Option<(Message, u32)> {
    if raw_msg.get_body().len() < 4 {
        return None;
    }

    let (mut header, mut payload) = raw_msg.split();
    let body = payload.split_off(4);
    let req_id = BigEndian::read_u32(&payload);

    if header.is_empty() {
        header = payload;
    } else {
        header.extend_from_slice(&payload);
    }

    Some((Message::from_header_and_body(header, body), req_id))
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

    use core::{EndpointId, Message, Scheduled};
    use core::socket::{Protocol, Reply};
    use core::context::{Event};
    use core::tests::*;

    use super::*;

    #[test]
    fn when_send_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(1);
        req.send(&mut ctx, msg, Some(timeout));
        req.on_send_ack(&mut ctx, eid);

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
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.send(&mut ctx, Message::new(), None);
        req.on_send_ack(&mut ctx, eid);
        req.on_send_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
        assert_eq!(Event::CanSend(true), raised_evts[2]);
    }

    #[test]
    fn when_send_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(2);
        let pipe = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanSend(true), raised_evts[0]);
        assert_eq!(Event::CanSend(false), raised_evts[1]);
    }

    #[test]
    fn when_active_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);
        let eid_o = EndpointId::from(2);
        let pipe_o = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.add_pipe(&mut ctx, eid_o, pipe_o);
        assert_eq!(0, ctx_sensor.borrow().get_raised_events().len());

        req.on_send_ready(&mut ctx, eid);
        assert_eq!(1, ctx_sensor.borrow().get_raised_events().len());
        assert_eq!(Event::CanSend(true), ctx_sensor.borrow().get_raised_events()[0]);

        req.send(&mut ctx, Message::new(), None);
        assert_eq!(2, ctx_sensor.borrow().get_raised_events().len());
        assert_eq!(Event::CanSend(false), ctx_sensor.borrow().get_raised_events()[1]);

        req.on_send_ack(&mut ctx, eid);
        assert_eq!(2, ctx_sensor.borrow().get_raised_events().len());

        req.on_recv_ready(&mut ctx, eid_o);
        assert_eq!(2, ctx_sensor.borrow().get_raised_events().len());

        req.on_recv_ready(&mut ctx, eid);
        assert_eq!(3, ctx_sensor.borrow().get_raised_events().len());
        assert_eq!(Event::CanRecv(true), ctx_sensor.borrow().get_raised_events()[2]);

        req.remove_pipe(&mut ctx, eid);
        assert_eq!(4, ctx_sensor.borrow().get_raised_events().len());
        assert_eq!(Event::CanRecv(false), ctx_sensor.borrow().get_raised_events()[3]);
    }

    #[test]
    fn when_in_regular_mode_send_will_append_request_id_to_the_header() {
        let (tx, _) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(4);
        let pipe = new_test_pipe(eid);
        let app_msg = Message::from_body(vec![1u8, 2, 3]);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.send(&mut ctx, app_msg, None);

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
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(5);
        let pipe = new_test_pipe(eid);
        let app_msg = Message::from_body(vec![1u8, 2, 3]);

        req.on_device_plugged(&mut ctx);
        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.send(&mut ctx, app_msg, None);

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
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);

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
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.send(&mut ctx, Message::new(), None);
        req.on_send_ack(&mut ctx, eid);
        let _ = rx.try_recv().expect("facade should have been sent a reply !");

        let bad_request_id = (req.inner.req_id_seq - 1) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], bad_request_id);

        let msg = Message::from_body(body);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);
        req.on_recv_ack(&mut ctx, eid, msg);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn when_in_regular_mode_recv_while_active_will_accept_msg_with_right_request_id() {
        let (tx, rx) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.send(&mut ctx, Message::new(), None);
        req.on_send_ack(&mut ctx, eid);
        let _ = rx.try_recv().expect("facade should have been sent a reply !");

        let good_request_id = (req.inner.req_id_seq) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], good_request_id);

        let msg = Message::from_body(body);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);
        req.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Recv(_) => true,
            _ => false
        };
        assert!(is_reply_ok);
    }

    #[test]
    fn when_in_regular_mode_recv_moves_the_request_id_from_the_body_to_the_header() {
        let (tx, rx) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.send(&mut ctx, Message::new(), None);
        req.on_send_ack(&mut ctx, eid);
        let _ = rx.try_recv().expect("facade should have been sent a reply !");

        let good_request_id = (req.inner.req_id_seq) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], good_request_id);

        let msg = Message::from_body(body);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);
        req.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.unwrap();
        assert_eq!(4, app_msg.get_header().len());
        assert_eq!(3, app_msg.get_body().len());
    }

    #[test]
    fn when_in_raw_mode_recv_while_active_will_accept_msg_with_any_request_id() {
        let (tx, rx) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        req.on_device_plugged(&mut ctx);
        req.add_pipe(&mut ctx, eid, pipe);
        req.on_send_ready(&mut ctx, eid);
        req.send(&mut ctx, Message::new(), None);
        req.on_send_ack(&mut ctx, eid);
        let _ = rx.try_recv().expect("facade should have been sent a reply !");

        let bad_request_id = (req.inner.req_id_seq + 666) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], bad_request_id);

        let msg = Message::from_body(body);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);
        req.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Recv(_) => true,
            _ => false
        };
        assert!(is_reply_ok);
    }

    #[test]
    fn when_in_raw_mode_recv_while_idle_will_succeed() {
        let (tx, rx) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        let request_id = (req.inner.req_id_seq + 666) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], request_id);

        let msg = Message::from_body(body);

        req.on_device_plugged(&mut ctx);
        req.add_pipe(&mut ctx, eid, pipe);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);
        req.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Recv(_) => true,
            _ => false
        };
        assert!(is_reply_ok);
    }

    #[test]
    fn when_in_raw_mode_recv_moves_the_request_id_from_the_body_to_the_header() {
        let (tx, rx) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        let request_id = (req.inner.req_id_seq + 666) | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], request_id);

        let msg = Message::from_body(body);

        req.on_device_plugged(&mut ctx);
        req.add_pipe(&mut ctx, eid, pipe);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);
        req.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.unwrap();
        assert_eq!(4, app_msg.get_header().len());
        assert_eq!(3, app_msg.get_body().len());
    }

    #[test]
    fn when_in_raw_mode_recv_will_accept_any_msg_with_a_four_bytes_header() {
        let (tx, rx) = mpsc::channel();
        let mut req = Req::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        let request_id = 666;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2];

        BigEndian::write_u32(&mut body[0..4], request_id);

        let msg = Message::from_body(body);

        req.on_device_plugged(&mut ctx);
        req.add_pipe(&mut ctx, eid, pipe);
        req.on_recv_ready(&mut ctx, eid);
        req.recv(&mut ctx, None);
        req.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.unwrap();
        assert_eq!(4, app_msg.get_header().len());
        assert_eq!(2, app_msg.get_body().len());
     }
}