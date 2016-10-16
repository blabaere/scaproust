// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
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
use core::context::{Context, Schedulable, Event};
use super::priolist::Priolist;
use super::{Timeout, REQ, REP};
use io_error::*;

pub struct Req {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Sending(EndpointId, Rc<Message>, Timeout, bool),
    SendOnHold(Rc<Message>, Timeout, bool),
    Active(PendingRequest),
    Receiving(PendingRequest, Timeout),
    RecvOnHold(PendingRequest, Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: HashMap<EndpointId, Pipe>,
    lb: Priolist,
    req_id_seq: u32,
    is_device_item: bool,
    resend_ivl: Duration
}

struct PendingRequest {
    eid: EndpointId,
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
            let new_state = transition(old_state, ctx, &mut self.inner);
            #[cfg(debug_assertions)] let new_name = new_state.name();

            self.state = Some(new_state);

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
        let pipe = self.inner.remove_pipe(eid);

        if pipe.is_some() {
            self.apply(ctx, |s, ctx, inner| s.on_pipe_removed(ctx, inner, eid));
        }

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
            State::Active(_)               => "Active",
            State::Receiving(_, _)         => "Receiving",
            State::RecvOnHold(_, _)        => "RecvOnHold"
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
            State::Receiving(p, timeout) => {
                if p.eid == eid {
                    State::Idle.recv(ctx, inner, timeout)
                } else {
                    State::Receiving(p, timeout)
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
        if let State::Active(p) = self {
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
                if id == eid {
                    let retry_timeout = inner.on_send_ack(ctx, timeout, retry);

                    State::Active(PendingRequest {
                        eid: eid,
                        req: msg,
                        retry_timeout: retry_timeout
                    })
                } else {
                    State::Sending(id, msg, timeout, retry)
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

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    fn recv(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout) -> State {
        if let State::Active(p) = self {
            State::Idle.recv_reply_for(ctx, inner, timeout, p)
        } else {
            inner.recv_when_inactive(ctx, timeout);

            State::Idle
        }
    }
    fn recv_reply_for(self, ctx: &mut Context, inner: &mut Inner, timeout: Timeout, p: PendingRequest) -> State {
        if inner.recv_from(ctx, p.eid) {
            State::Receiving(p, timeout)
        } else {
            State::RecvOnHold(p, timeout)
        }
    }
    fn on_recv_ack(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId, msg: Message, req_id: u32) -> State {
        match self {
            State::Receiving(p, timeout) => {
                if p.eid == eid {
                    if inner.cur_req_id() == req_id {
                        inner.on_recv_ack(ctx, timeout, msg, p.retry_timeout);
                        State::Idle
                    } else {
                        State::Idle.recv_reply_for(ctx, inner, timeout, p)
                    }
                } else {
                    State::Receiving(p, timeout)
                }
            },
            any => any
        }
    }
    fn on_recv_timeout(self, ctx: &mut Context, inner: &mut Inner) -> State {
        match self {
            State::Receiving(p, _) |
            State::RecvOnHold(p, _) => inner.on_recv_timeout(ctx, p.retry_timeout),
            _ => {}
        }

        State::Idle
    }
    fn on_recv_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        match self {
            State::RecvOnHold(p, timeout) => {
                if eid == p.eid {
                    State::Active(p).recv(ctx, inner, timeout)
                } else {
                    State::RecvOnHold(p, timeout)
                }
            },
            any => {
                ctx.raise(Event::CanRecv(true));
                any
            }
        }
    }
    fn on_retry_timeout(self, ctx: &mut Context, inner: &mut Inner) -> State {
        if let State::Active(p) = self {
            State::Idle.send(ctx, inner, p.req, None, true)
        } else {
            self
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
            lb: Priolist::new(),
            req_id_seq: time::get_time().nsec as u32,
            is_device_item: false,
            resend_ivl: Duration::from_secs(60)
        }
    }
    fn add_pipe(&mut self, eid: EndpointId, pipe: Pipe) {
        self.lb.insert(eid, pipe.get_send_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.lb.remove(&eid);
        self.pipes.remove(&eid)
    }
    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>) -> Option<EndpointId> {
        self.lb.pop().map_or(None, |eid| self.send_to(ctx, msg, eid))
    }
    fn send_to(&mut self, ctx: &mut Context, msg: Rc<Message>, eid: EndpointId) -> Option<EndpointId> {
        self.pipes.get_mut(&eid).map_or(None, |pipe| {
            pipe.send(ctx, msg); 
            Some(eid)
        })
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
        ctx.schedule(Schedulable::ReqResend, self.resend_ivl).ok()
    }
    fn on_send_timeout(&self) {
        let error = timedout_io_error("Send timed out");
        let _ = self.reply_tx.send(Reply::Err(error));
    }
    fn cancel(&self, ctx: &mut Context, p: PendingRequest) {
        if let Some(sched) = p.retry_timeout {
            ctx.cancel(sched);
        }
    }

    fn recv_from(&mut self, ctx: &mut Context, eid: EndpointId) -> bool {
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

    fn msg_to_raw_msg(&mut self, msg: Message) -> Message {
        if self.is_device_item {
            msg
        } else {
            encode(msg, self.next_req_id())
        }
    }

    fn raw_msg_to_msg(&self, raw_msg: Message) -> Option<(Message, u32)> {
        if self.is_device_item {
            Some((raw_msg, self.cur_req_id()))
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
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
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
