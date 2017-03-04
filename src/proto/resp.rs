// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use byteorder::*;

use core::{EndpointId, Message};
use core::socket::{Protocol, Reply};
use core::endpoint::Pipe;
use core::context::{Context, Event};
use super::priolist::Priolist;
use super::pipes::PipeCollection;
use super::{Timeout, SURVEYOR, RESPONDENT};
use super::policy::fair_queue;
use io_error::*;

pub struct Respondent {
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
    pipes: PipeCollection,
    fq: Priolist,
    sd: HashSet<EndpointId>,
    ttl: u8,
    backtrace: Vec<u8>,
    is_device_item: bool
}

/*****************************************************************************/
/*                                                                           */
/* Respondent                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Respondent {

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

impl From<Sender<Reply>> for Respondent {
    fn from(tx: Sender<Reply>) -> Respondent {
        Respondent {
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

impl Protocol for Respondent {
    fn id(&self)      -> u16 { RESPONDENT }
    fn peer_id(&self) -> u16 { SURVEYOR }

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
            any => {
                ctx.raise(Event::CanRecv(true));
                any
            }
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
            pipes: PipeCollection::new(),
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
        self.pipes.send_to(ctx, msg, eid).is_some()
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
        fair_queue::recv(&mut self.fq, &mut self.pipes, ctx)
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
        self.pipes.close_all(ctx)
    }
}
