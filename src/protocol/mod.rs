// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::io;

use mio;

use global::{ SocketType, SocketId, other_io_error };
use event_loop_msg::{ SocketNotify, SocketEvtSignal, SocketOption };
use pipe::Pipe;
use EventLoop;
use Message;

pub mod excl;
pub mod priolist;

pub mod push;
pub mod pull;
pub mod pair;
pub mod req;
//pub mod rep;
//pub mod pbu;
//pub mod sub;
//pub mod bus;
//pub mod surv;
//pub mod resp;

//pub mod sender;
//pub mod receiver;

pub fn create_protocol(socket_id: SocketId, socket_type: SocketType, evt_tx: Rc<mpsc::Sender<SocketNotify>>) -> Box<Protocol> {
    match socket_type {
        SocketType::Push       => Box::new(push::Push::new(socket_id, evt_tx)),
        SocketType::Pull       => Box::new(pull::Pull::new(socket_id, evt_tx)),
        SocketType::Pair       => Box::new(pair::Pair::new(socket_id, evt_tx)),
        SocketType::Req        => Box::new(req::Req::new(socket_id, evt_tx)),
        SocketType::Rep        => Box::new(NullProtocol),
        SocketType::Pub        => Box::new(NullProtocol),
        SocketType::Sub        => Box::new(NullProtocol),
        SocketType::Bus        => Box::new(NullProtocol),
        SocketType::Surveyor   => Box::new(NullProtocol),
        SocketType::Respondent => Box::new(NullProtocol)
    }
}

pub trait Protocol {
    fn id(&self) -> u16;
    fn peer_id(&self) -> u16;

    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) -> io::Result<()>;
    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe>;

    fn register_pipe(&mut self, event_loop: &mut EventLoop, token: mio::Token);
    fn on_pipe_register(&mut self, event_loop: &mut EventLoop, token: mio::Token);

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
    fn on_request_timeout(&mut self, _: &mut EventLoop) {}
}

fn clear_timeout(event_loop: &mut EventLoop, handle: Option<mio::Timeout>) {
    if let Some(timeout) = handle {
        event_loop.clear_timeout(timeout);
    }
}

struct NullProtocol;

impl Protocol for NullProtocol {
    fn id(&self) -> u16 {
        0
    }
    fn peer_id(&self) -> u16 {
        0
    }

    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) -> io::Result<()> {
        Err(other_io_error("not implemented"))
    }
    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
        None
    }

    fn register_pipe(&mut self, event_loop: &mut EventLoop, token: mio::Token) {}
    fn on_pipe_register(&mut self, event_loop: &mut EventLoop, token: mio::Token) {}

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) {
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout_handle: Option<mio::Timeout>) {}
    fn on_send_by_pipe(&mut self, event_loop: &mut EventLoop, token: mio::Token) {}
    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {}

    fn recv(&mut self, event_loop: &mut EventLoop, timeout_handle: Option<mio::Timeout>) {}
    fn on_recv_by_pipe(&mut self, event_loop: &mut EventLoop, tok: mio::Token, msg: Message) {}
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {}
}