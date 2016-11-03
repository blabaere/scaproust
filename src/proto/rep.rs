// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::Sender;

use byteorder::*;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::endpoint::Pipe;
use core::context::Context;
use super::priolist::Priolist;
use super::{Timeout, REQ, REP};
use io_error::*;

pub struct Rep {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Receiving(EndpointId, Timeout),
    RecvOnHold(Timeout),
    Active(EndpointId),
    Sending(EndpointId, Rc<Message>, Timeout),
    SendOnHold(EndpointId, Rc<Message>, Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: HashMap<EndpointId, Pipe>,
    fq: Priolist,
    sd: HashSet<EndpointId>,
    ttl: u8,
    backtrace: Vec<u8>,
    is_device_item: bool
}

/*****************************************************************************/
/*                                                                           */
/* Rep                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Rep {

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

impl From<Sender<Reply>> for Rep {
    fn from(tx: Sender<Reply>) -> Rep {
        Rep {
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

impl Protocol for Rep {
    fn id(&self)      -> u16 { REP }
    fn peer_id(&self) -> u16 { REQ }

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
        if let Some((raw_msg, eid)) = self.inner.msg_to_raw_msg(msg) {
            self.apply(ctx, |s, ctx, inner| s.send(ctx, inner, Rc::new(raw_msg), timeout, eid))
        } else {
            self.inner.on_send_malformed(ctx, timeout);
        }
    }
    fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.inner.clear_backtrace();
        
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
        if let Some(msg) = self.inner.raw_msg_to_msg(raw_msg, eid) {
            self.apply(ctx, |s, ctx, inner| s.on_recv_ack(ctx, inner, eid, msg))
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
            State::Sending(..)    => "Sending",
            State::SendOnHold(..) => "SendOnHold",
            State::Active(..)     => "Active",
            State::Receiving(..)  => "Receiving",
            State::RecvOnHold(..) => "RecvOnHold"
        }
    }

    fn on_pipe_removed(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Receiving(id, timeout) => {
                if id == eid {
                    State::Idle.recv(ctx, inner, timeout)
                } else {
                    State::Receiving(id, timeout)
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

    fn send(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Timeout, eid: EndpointId) -> State {
        if inner.is_device_item {
            if inner.is_send_ready_to(&eid) {
                State::Idle.send_reply_to(ctx, inner, msg, timeout, eid)
            } else {
                inner.on_send_ack(ctx, timeout);

                State::Idle
            }
        } else if let State::Active(_) = self {
            if inner.is_send_ready_to(&eid) {
                State::Idle.send_reply_to(ctx, inner, msg, timeout, eid)
            } else {
                State::SendOnHold(eid, msg, timeout)
            }
        } else {
            inner.send_when_inactive(ctx, timeout);

            State::Idle
        }
    }
    fn send_reply_to(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, timeout: Timeout, eid: EndpointId) -> State {
        if inner.send_to(ctx, msg.clone(), eid) {
            State::Sending(eid, msg, timeout)
        } else {
            State::SendOnHold(eid, msg, timeout)
        }
    }
    fn on_send_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::Sending(id, msg, timeout) => {
                if id == eid {
                    inner.on_send_ack(ctx, timeout);
                    State::Idle
                } else {
                    State::Sending(id, msg, timeout)
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
            State::SendOnHold(id, msg, timeout) => {
                if id == eid {
                    State::Idle.send_reply_to(ctx, inner, msg, timeout, eid)
                } else {
                    State::SendOnHold(id, msg, timeout)
                }
            },
            any => any
        }
    }
    fn is_send_ready(&self, inner: &Inner) -> bool {
        if inner.is_device_item {
            inner.is_send_ready()
        }
        else if let State::Active(ref eid) = *self {
            inner.is_send_ready_to(eid)
        } else {
            false
        }
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn recv(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout) -> State {
        inner.recv(ctx).map_or_else(
            |   | State::RecvOnHold(timeout),
            |eid| State::Receiving(eid, timeout))
    }
    fn on_recv_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId, msg: Message) -> State {
        match self {
            State::Receiving(id, timeout) => {
                if id == eid {
                    inner.on_recv_ack(ctx, timeout, msg);
                    if inner.is_device_item {
                        State::Idle
                    } else {
                        State::Active(eid)
                    }
                    
                } else {
                    State::Receiving(id, timeout)
                }
            },
            any => any
        }
    }
    fn on_recv_timeout(self, _: &mut Context, inner: &mut Inner) -> State {
        inner.on_recv_timeout();

        State::Idle
    }
    fn on_recv_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_recv_ready(eid);

        match self {
            State::RecvOnHold(timeout) => State::Idle.recv(ctx, inner, timeout),
            any => any
        }
    }
    fn is_recv_ready(&self, inner: &Inner) -> bool {
        inner.is_recv_ready()
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
            fq: Priolist::new(),
            sd: HashSet::new(),
            ttl: 8,
            backtrace: Vec::new(),
            is_device_item: false
        }
    }
    fn add_pipe(&mut self, eid: EndpointId, pipe: Pipe) {
        self.fq.insert(eid, pipe.get_recv_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.fq.remove(&eid);
        self.sd.remove(&eid);
        self.pipes.remove(&eid)
    }
    fn on_send_malformed(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = invalid_data_io_error("Sending without eid");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn send_to(&mut self, ctx: &mut Context, msg: Rc<Message>, eid: EndpointId) -> bool {
        self.sd.remove(&eid);
        self.pipes.get_mut(&eid).map(|pipe| pipe.send(ctx, msg)).is_some()
    }
    fn on_send_ack(&self, ctx: &mut Context, timeout: Timeout) {
        let _ = self.reply_tx.send(Reply::Send);
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn send_when_inactive(&mut self, ctx: &mut Context, timeout: Timeout) {
        let error = other_io_error("Can't send: no active request");
        let _ = self.reply_tx.send(Reply::Err(error));
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }
    fn on_send_timeout(&self) {
        let error = timedout_io_error("Send timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn on_send_ready(&mut self, eid: EndpointId) {
        self.sd.insert(eid);
    }
    fn is_send_ready(&self) -> bool {
        !self.sd.is_empty()
    }
    fn is_send_ready_to(&self, eid: &EndpointId) -> bool {
        self.sd.contains(eid)
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
    fn on_recv_ready(&mut self, eid: EndpointId) {
        self.fq.activate(&eid)
    }
    fn on_recv_ack(&mut self, ctx: &mut Context, timeout: Timeout, mut msg: Message) {
        if !self.is_device_item {
            self.set_backtrace(&msg.header);
            msg.header.clear();
        }
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
        let error = invalid_data_io_error("Received request without id");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn is_recv_ready(&self) -> bool {
        self.fq.peek()
    }
 
    fn raw_msg_to_msg(&self, raw_msg: Message, eid: EndpointId) -> Option<Message> {
        let (mut header, mut body) = raw_msg.split();
        let mut hops = 0;
        let mut eid_bytes: [u8; 4] = [0; 4];
        let eid_usize: usize = eid.into();

        BigEndian::write_u32(&mut eid_bytes[0..4], eid_usize as u32);

        header.reserve(4);
        header.extend_from_slice(&eid_bytes[..]);

        loop {
            if hops >= self.ttl {
                return None;
            }
            hops += 1;

            if body.len() < 4 {
                return None;
            }

            let tail = body.split_off(4);
            header.reserve(4);
            header.extend_from_slice(&body);

            let position = header.len() - 4;
            if header[position] & 0x80 != 0 {
                return Some(Message::from_header_and_body(header, tail));
            }
            body = tail;
        }
    }
    fn msg_to_raw_msg(&self, msg: Message) -> Option<(Message, EndpointId)> {
        let (mut header, body) = msg.split();

        if !self.is_device_item {
            let backtrace = self.get_backtrace();

            header.clear();
            header.extend_from_slice(backtrace);
        }

        if header.len() < 4 {
            return None;
        }

        let tail = header.split_off(4);
        let eid_u32 = BigEndian::read_u32(&header);
        let eid = EndpointId::from(eid_u32 as usize);
        
        Some((Message::from_header_and_body(tail, body), eid))
    }
    fn set_backtrace(&mut self, bt: &[u8]) {
        self.backtrace.clear();
        self.backtrace.extend_from_slice(bt);
    }
    fn get_backtrace(&self) -> &[u8] {
        &self.backtrace
    }
    fn clear_backtrace(&mut self) {
        self.backtrace.clear();
    }
    fn close(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
    }
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
    fn when_recv_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);
        let timeout = Scheduled::from(1);
        let request_id = 666 | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], request_id);

        let msg = Message::from_body(body);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, Some(timeout));
        rep.on_recv_ack(&mut ctx, eid, msg);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Recv(_) => true,
            _ => false
        };
        assert!(is_reply_ok);

        let sensor = ctx_sensor.borrow();
        sensor.assert_one_recv_from(eid);
        sensor.assert_one_cancellation(timeout);
    }

    #[test]
    fn send_before_recv_notifies_an_error() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_send_ready(&mut ctx, eid);

        let msg = Message::new();
        let timeout = Scheduled::from(1);
        rep.send(&mut ctx, msg, Some(timeout));
        rep.on_send_ack(&mut ctx, eid);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_err = match reply {
            Reply::Err(..) => true,
            _ => false
        };
        assert!(is_reply_err);

        let sensor = ctx_sensor.borrow();
        sensor.assert_no_send_call();
        sensor.assert_one_cancellation(timeout);
    }

    #[test]
    fn when_send_succeed_it_is_notified_and_timeout_is_cancelled() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);
        let timeout = Scheduled::from(1);
        let request_id = 666 | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], request_id);

        let msg = Message::from_body(body);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, msg);
        let _ = rx.recv(); // flush recv reply

        rep.on_send_ready(&mut ctx, eid);
        rep.send(&mut ctx, Message::new(), Some(timeout));
        rep.on_send_ack(&mut ctx, eid);

        let reply = rx.recv().expect("facade should have been sent a reply !");
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
    fn when_recv_starts_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, Message::new());
        rep.on_recv_ready(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(3, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
        assert_eq!(Event::CanRecv(true), raised_evts[2]);
    }

    #[test]
    fn when_send_starts_event_is_raised() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);

        let request_id = 666 | 0x80000000;
        let mut body: Vec<u8> = vec![0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[0..4], request_id);

        let msg = Message::from_body(body);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, msg);
        let _ = rx.recv(); // flush recv reply

        rep.on_send_ready(&mut ctx, eid);
        rep.send(&mut ctx, Message::new(), None);
        rep.on_send_ack(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(4, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
        assert_eq!(Event::CanSend(true), raised_evts[2]);
        assert_eq!(Event::CanSend(false), raised_evts[3]);
    }

    #[test]
    fn when_recv_ready_pipe_is_removed_event_is_raised() {
        let (tx, _) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(2);
        let pipe = new_test_pipe(eid);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.remove_pipe(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        let raised_evts = sensor.get_raised_events();

        assert_eq!(2, raised_evts.len());
        assert_eq!(Event::CanRecv(true), raised_evts[0]);
        assert_eq!(Event::CanRecv(false), raised_evts[1]);
    }

    #[test]
    fn when_in_regular_mode_recv_will_store_endpoint_and_backtrace_in_socket_state() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);
        let request_id = 666 | 0x80000000;
        let mut body: Vec<u8> = vec![0, 2, 4, 2, 0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[4..8], request_id);

        let msg = Message::from_body(body);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, msg);
        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.unwrap();
        assert_eq!(0, app_msg.get_header().len());
        assert_eq!(3, app_msg.get_body().len());
        assert_eq!(12, rep.inner.get_backtrace().len());
    }

    #[test]
    fn when_in_raw_mode_recv_will_store_endpoint_and_backtrace_in_msg_header() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);
        let request_id = 666 | 0x80000000;
        let mut body: Vec<u8> = vec![0, 2, 4, 2, 0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[4..8], request_id);

        let msg = Message::from_body(body);

        rep.on_device_plugged(&mut ctx);
        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, msg);
        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.unwrap();
        assert_eq!(12, app_msg.get_header().len());
        assert_eq!(3, app_msg.get_body().len());
        assert_eq!(0, rep.inner.get_backtrace().len());
    }

    #[test]
    fn when_in_regular_mode_send_will_restore_backtrace_from_socket_state_in_header_before_removing_endoint_id() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(0);
        let pipe = new_test_pipe(eid);
        let request_id = 666 | 0x80000000;
        let mut body: Vec<u8> = vec![0, 2, 4, 2, 0, 0, 0, 0, 4, 2, 1];

        BigEndian::write_u32(&mut body[4..8], request_id);

        let msg = Message::from_body(body);

        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_recv_ready(&mut ctx, eid);
        rep.recv(&mut ctx, None);
        rep.on_recv_ack(&mut ctx, eid, msg);
        let reply = rx.try_recv().expect("facade should have been sent a reply !");
        let reply_msg = match reply {
            Reply::Recv(msg) => Some(msg),
            _ => None
        };
        let app_msg = reply_msg.unwrap();
        assert_eq!(0, app_msg.get_header().len());
        assert_eq!(3, app_msg.get_body().len());
        assert_eq!(12, rep.inner.get_backtrace().len());

        rep.on_send_ready(&mut ctx, eid);
        rep.send(&mut ctx, Message::new(), None);
        rep.on_send_ack(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        sensor.assert_one_send_to(eid);

        assert_eq!(0, rep.inner.get_backtrace().len());
    }
    
    #[test]
    fn when_in_raw_mode_send_will_remove_endpoint_id_from_header() {
        let (tx, _) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);
        let header: Vec<u8> = vec![0, 0, 0, 1];
        let body: Vec<u8> = vec![6, 6, 6, 6, 4, 2, 1];
        let msg = Message::from_header_and_body(header, body);

        rep.on_device_plugged(&mut ctx);
        rep.add_pipe(&mut ctx, eid, pipe);
        rep.on_send_ready(&mut ctx, eid);
        rep.send(&mut ctx, msg, None);
        rep.on_send_ack(&mut ctx, eid);

        let sensor = ctx_sensor.borrow();
        sensor.assert_one_send_to(eid);

        let send_calls = sensor.get_send_calls();

        assert_eq!(1, send_calls.len());
        let &(_, ref proto_msg) = &send_calls[0];
        let header = proto_msg.get_header();
        let body = proto_msg.get_body();

        assert_eq!(0, header.len());
        assert_eq!(7, body.len());
    }

    #[test]
    fn when_in_raw_mode_send_while_peer_is_not_ready_drops_the_message_and_reports_success() {
        let (tx, rx) = mpsc::channel();
        let mut rep = Rep::from(tx);
        let ctx_sensor = Rc::new(RefCell::new(TestContextSensor::default()));
        let mut ctx = TestContext::with_sensor(ctx_sensor.clone());
        let eid = EndpointId::from(1);
        let pipe = new_test_pipe(eid);
        let header: Vec<u8> = vec![0, 0, 0, 1];
        let body: Vec<u8> = vec![6, 6, 6, 6, 4, 2, 1];
        let msg = Message::from_header_and_body(header, body);

        rep.on_device_plugged(&mut ctx);
        rep.add_pipe(&mut ctx, eid, pipe);
        rep.send(&mut ctx, msg, None);
        rep.on_send_ack(&mut ctx, eid);

        let reply = rx.recv().expect("facade should have been sent a reply !");
        let is_reply_ok = match reply {
            Reply::Send => true,
            _ => false
        };
        assert!(is_reply_ok);

        ctx_sensor.borrow().assert_no_send_call();
    }
}
