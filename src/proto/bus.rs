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
use core::context::{Context, Event};
use super::priolist::Priolist;
use super::{Timeout, BUS};
use io_error::*;

pub struct Bus {
    inner: Inner,
    state: Option<State>
}

enum State {
    Idle,
    Receiving(EndpointId, Timeout),
    RecvOnHold(Timeout)
}

struct Inner {
    reply_tx: Sender<Reply>,
    pipes: HashMap<EndpointId, Pipe>,
    bc: HashSet<EndpointId>,
    fq: Priolist,
}

/*****************************************************************************/
/*                                                                           */
/* Bus                                                                      */
/*                                                                           */
/*****************************************************************************/

impl Bus {

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

impl From<Sender<Reply>> for Bus {
    fn from(tx: Sender<Reply>) -> Bus {
        Bus {
            inner: Inner {
                reply_tx: tx,
                pipes: HashMap::new(),
                bc: HashSet::new(),
                fq: Priolist::new()
            },
            state: Some(State::Idle)
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Protocol                                                                  */
/*                                                                           */
/*****************************************************************************/

impl Protocol for Bus {
    fn id(&self)      -> u16 { BUS }
    fn peer_id(&self) -> u16 { BUS }

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
        let (raw_msg, oid) = encode(msg);

        self.apply(ctx, |s, ctx, inner| s.send(ctx, inner, Rc::new(raw_msg), oid, timeout))
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
        let msg = decode(raw_msg, eid);
        self.apply(ctx, |s, ctx, inner| s.on_recv_ack(ctx, inner, eid, msg))
    }
    fn on_recv_timeout(&mut self, ctx: &mut Context) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_timeout(ctx, inner))
    }
    fn on_recv_ready(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.apply(ctx, |s, ctx, inner| s.on_recv_ready(ctx, inner, eid))
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
            State::Idle            => "Idle",
            State::Receiving(_, _) => "Receiving",
            State::RecvOnHold(_)   => "RecvOnHold"
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

    fn send(self, ctx: &mut Context, inner: &mut Inner, msg: Rc<Message>, oid: Option<EndpointId>, timeout: Timeout) -> State {
        inner.send(ctx, msg, oid, timeout);
        self
    }
    fn on_send_ack(self, _: &mut Context, _: &mut Inner, _: EndpointId) -> State {
        self
    }
    fn on_send_timeout(self, _: &mut Context, _: &mut Inner) -> State {
        self
    }
    fn on_send_ready(self, ctx: &mut Context, inner: &mut Inner, eid: EndpointId) -> State {
        inner.on_send_ready(ctx, eid);
        self
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
                    State::Idle
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
                ctx.raise(Event::CanRecv);
                any
            }
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Inner                                                                     */
/*                                                                           */
/*****************************************************************************/

impl Inner {
    fn add_pipe(&mut self, eid: EndpointId, pipe: Pipe) {
        self.fq.insert(eid, pipe.get_recv_priority());
        self.pipes.insert(eid, pipe);
    }
    fn remove_pipe(&mut self, eid: EndpointId) -> Option<Pipe> {
        self.bc.remove(&eid);
        self.fq.remove(&eid);
        self.pipes.remove(&eid)
    }

    fn send(&mut self, ctx: &mut Context, msg: Rc<Message>, oid: Option<EndpointId>, timeout: Timeout) {
        if let Some(except) = oid {
            for id in self.bc.drain().filter(|x| *x != except) {
                self.pipes.get_mut(&id).map(|pipe| pipe.send(ctx, msg.clone()));
            }
        } else {
            for id in self.bc.drain() {
                self.pipes.get_mut(&id).map(|pipe| pipe.send(ctx, msg.clone()));
            }
        }

        let _ = self.reply_tx.send(Reply::Send);
        if let Some(sched) = timeout {
            ctx.cancel(sched);
        }
    }

    fn on_send_ready(&mut self, _: &mut Context, eid: EndpointId) {
        self.bc.insert(eid);
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
    fn close(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* Codec                                                                     */
/*                                                                           */
/*****************************************************************************/

fn decode(raw_msg: Message, eid: EndpointId) -> Message {
    let originator: usize = eid.into();
    let mut msg = raw_msg;
    let mut originator_bytes: [u8; 4] = [0; 4];

    BigEndian::write_u32(&mut originator_bytes[..], originator as u32);

    msg.header.reserve(4);
    msg.header.extend_from_slice(&originator_bytes);
    msg
}

fn encode(msg: Message) -> (Message, Option<EndpointId>) {
    if msg.get_header().len() < 4 {
        return (msg, None);
    }

    let (mut header, body) = msg.split();
    let remaining_header = header.split_off(4);
    let originator = BigEndian::read_u32(&header) as usize;
    let raw_msg = Message::from_header_and_body(remaining_header, body);

    (raw_msg, Some(EndpointId::from(originator)))
}
