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
use EventLoopAction;
use Message;

mod priolist;

//pub mod push;
//pub mod pull;
pub mod pair;
//pub mod req;
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
        SocketType::Push       => Box::new(NullProtocol),
        SocketType::Pull       => Box::new(NullProtocol),
        SocketType::Pair       => Box::new(pair::Pair::new(evt_tx)),
        SocketType::Req        => Box::new(NullProtocol),
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

    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe);
    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe>;

    fn open_pipe(&mut self, event_loop: &mut EventLoop, token: mio::Token);

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet);
    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction);
    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction);

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop);
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop);
    
    fn set_option(&mut self, event_loop: &mut EventLoop, option: SocketOption) -> io::Result<()>;
    fn on_survey_timeout(&mut self, event_loop: &mut EventLoop);
    fn on_request_timeout(&mut self, event_loop: &mut EventLoop);
}

struct NullProtocol;

impl Protocol for NullProtocol {
    fn id(&self) -> u16 {
        0
    }
    fn peer_id(&self) -> u16 {
        0
    }

    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {}
    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
        None
    }

    fn open_pipe(&mut self, event_loop: &mut EventLoop, token: mio::Token) {}

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) {
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {}
    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {}

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {}
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {}

    fn set_option(&mut self, event_loop: &mut EventLoop, option: SocketOption) -> io::Result<()> {
        Err(other_io_error("not implemented"))
    }

    fn on_survey_timeout(&mut self, event_loop: &mut EventLoop) {}
    fn on_request_timeout(&mut self, event_loop: &mut EventLoop) {}
}