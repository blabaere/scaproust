// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::io;

use mio;

use global::{ SocketType, SocketId };
use event_loop_msg::{ SocketNotify, SocketOption };
use pipe::Pipe;
use EventLoop;
use Message;

pub type Timeout = Option<mio::Timeout>;

pub mod excl;
pub mod priolist;

mod with_notify;
mod with_pipes;
mod with_fair_queue;
mod with_load_balancing;
mod with_unicast_send;
mod with_unicast_recv;
mod without_send;
mod without_recv;
mod with_backtrace;

pub mod push;
pub mod pull;
pub mod pair;
pub mod req;
pub mod rep;
pub mod pbu;
pub mod sub;
pub mod surv;
pub mod resp;
pub mod bus;

pub fn create_protocol(socket_id: SocketId, socket_type: SocketType, evt_tx: Rc<mpsc::Sender<SocketNotify>>) -> Box<Protocol> {
    match socket_type {
        SocketType::Push       => box push::Push::new(socket_id, evt_tx),
        SocketType::Pull       => box pull::Pull::new(socket_id, evt_tx),
        SocketType::Pair       => box pair::Pair::new(socket_id, evt_tx),
        SocketType::Req        => box req::Req::new(socket_id, evt_tx),
        SocketType::Rep        => box rep::Rep::new(socket_id, evt_tx),
        SocketType::Pub        => box pbu::Pub::new(socket_id, evt_tx),
        SocketType::Sub        => box sub::Sub::new(socket_id, evt_tx),
        SocketType::Bus        => box bus::Bus::new(socket_id, evt_tx),
        SocketType::Surveyor   => box surv::Surv::new(socket_id, evt_tx),
        SocketType::Respondent => box resp::Resp::new(socket_id, evt_tx)
    }
}

pub trait Protocol {

    fn get_type(&self) -> SocketType;

    fn id(&self) -> u16 {
        self.get_type().id()
    }
    fn peer_id(&self) -> u16 {
        self.get_type().peer().id()
    }

    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) -> io::Result<()>;
    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe>;

    fn open_pipe(&mut self, event_loop: &mut EventLoop, token: mio::Token);
    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, token: mio::Token);

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet);

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout_handle: Option<mio::Timeout>);
    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token);
    fn on_send_timeout(&mut self, event_loop: &mut EventLoop);

    fn recv(&mut self, event_loop: &mut EventLoop, timeout_handle: Option<mio::Timeout>);
    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token, msg: Message);
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop);

    fn set_option(&mut self, _: &mut EventLoop, _: SocketOption) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "option not supported by protocol"))
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {}
    fn resend(&mut self, _: &mut EventLoop) {}

    fn destroy(&mut self, event_loop: &mut EventLoop);
}

fn clear_timeout(event_loop: &mut EventLoop, handle: Option<mio::Timeout>) {
    if let Some(timeout) = handle {
        event_loop.clear_timeout(&timeout);
    }
}
