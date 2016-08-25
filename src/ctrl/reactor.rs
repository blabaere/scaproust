// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;

use mio;

use core::{session, socket, endpoint};

use transport::{PipeCmd, PipeEvt, AcceptorCmd, AcceptorEvt};

use ctrl::signal::Signal;
use ctrl::bus::EventLoopBus;
use ctrl::adapter::{
    EndpointCollection,
    SocketEventLoopContext
};
use sequence::Sequence;

/// Requests flowing to core components via the controller
pub enum Request {
    Session(session::Request),
    Socket(socket::SocketId, socket::Request),
    Endpoint(socket::SocketId, endpoint::EndpointId, endpoint::Request)
}

const BUS_TOKEN: mio::Token = mio::Token(::std::usize::MAX - 3);

#[doc(hidden)]
pub type EventLoop = mio::EventLoop<EventLoopHandler>;

pub fn run_event_loop(mut event_loop: EventLoop, reply_tx: mpsc::Sender<session::Reply>) {
    let signal_bus = EventLoopBus::new();
    let signal_bus_reg = register_signal_bus(&mut event_loop, &signal_bus);
    let mut handler = EventLoopHandler::new(signal_bus, reply_tx);
    let exec = signal_bus_reg.and_then(|_| event_loop.run(&mut handler));

    /*match exec {
        Ok(_) => debug!("event loop exited"),
        Err(e) => error!("event loop failed to run: {}", e)
    }*/
}

fn register_signal_bus(event_loop: &mut EventLoop, signal_bus: &EventLoopBus<Signal>) -> io::Result<()> {
    event_loop.register(signal_bus, BUS_TOKEN, mio::EventSet::readable(), mio::PollOpt::edge())
}

pub struct EventLoopHandler {
    signal_bus: EventLoopBus<Signal>,
    session: session::Session,
    endpoints: EndpointCollection
}

impl EventLoopHandler {
    fn new(bus: EventLoopBus<Signal>, reply_tx: mpsc::Sender<session::Reply>) -> EventLoopHandler {
        let seq = Sequence::new();
        EventLoopHandler {
            signal_bus: bus,
            session: session::Session::new(seq.clone(), reply_tx),
            endpoints: EndpointCollection::new(seq.clone())
        }
    }
    fn process_request(&mut self, request: Request) {
        match request {
            Request::Session(req) => self.process_session_request(req),
            Request::Socket(id, req) => self.process_socket_request(id, req),
            _ => {}
        }
    }
    fn process_session_request(&mut self, request: session::Request) {
        match request {
            session::Request::CreateSocket(ctor) => self.session.add_socket(ctor),
            _ => {}
        }
    }
    fn process_socket_request(&mut self, id: socket::SocketId, request: socket::Request) {
        match request {
            socket::Request::Connect(url) => self.apply_on_socket(id, |socket, network| socket.connect(network, url)),
            socket::Request::Bind(url)    => self.apply_on_socket(id, |socket, network| socket.bind(network, url)),
            socket::Request::Send(msg)    => self.apply_on_socket(id, |socket, network| socket.send(network, msg)),
            socket::Request::Recv         => self.apply_on_socket(id, |socket, network| socket.recv(network)),
            _ => {}
        }
    }
    fn apply_on_socket<F>(&mut self, id: socket::SocketId, f: F) 
    where F : FnOnce(&mut socket::Socket, &mut SocketEventLoopContext) {
        if let Some(socket) = self.session.get_socket_mut(id) {
            let mut ctx = SocketEventLoopContext::new(id, &mut self.signal_bus, &mut self.endpoints);

            f(socket, &mut ctx);
        }
    }
    fn process_endpoint_readiness(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        let eid = endpoint::EndpointId::from(tok);
        if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
            pipe.ready(event_loop, &mut self.signal_bus, events);
        }
    }
    fn process_signal_bus_readiness(&mut self, event_loop: &mut EventLoop) {
        while let Some(signal) = self.signal_bus.recv() {
            self.process_signal(event_loop, signal);
        }
    }
    fn process_signal(&mut self, event_loop: &mut EventLoop, signal: Signal) {
        match signal {
            Signal::PipeCmd(_, eid, cmd) => self.process_pipe_cmd(event_loop, eid, cmd),
            Signal::AcceptorCmd(_, eid, cmd) => self.process_acceptor_cmd(event_loop, eid, cmd),
            Signal::PipeEvt(sid, eid, cmd) => self.process_pipe_evt(sid, eid, cmd),
            Signal::AcceptorEvt(sid, eid, cmd) => self.process_acceptor_evt(event_loop, sid, eid, cmd)
        }
    }
    fn process_pipe_cmd(&mut self, event_loop: &mut EventLoop, eid: endpoint::EndpointId, cmd: PipeCmd) {
        if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
            pipe.process(event_loop, &mut self.signal_bus, cmd);
        }
    }
    fn process_pipe_evt(&mut self, sid: socket::SocketId, eid: endpoint::EndpointId, evt: PipeEvt) {
        match evt {
            PipeEvt::Opened => self.apply_on_socket(sid, |socket, ctx| socket.on_pipe_opened(ctx, eid)),
            PipeEvt::Sent   => self.apply_on_socket(sid, |socket, ctx| socket.on_send_ack(ctx, eid)),
            _ => {}
        }
    }
    fn process_acceptor_cmd(&mut self, event_loop: &mut EventLoop, eid: endpoint::EndpointId, cmd: AcceptorCmd) {
        if let Some(acceptor) = self.endpoints.get_acceptor_mut(eid) {
            acceptor.process(event_loop, &mut self.signal_bus, cmd);
        }
    }
    fn process_acceptor_evt(&mut self, event_loop: &mut EventLoop, sid: socket::SocketId, eid: endpoint::EndpointId, cmd: AcceptorEvt) {
    }
}

impl mio::Handler for EventLoopHandler {
    type Timeout = ();
    type Message = Request;

    fn notify(&mut self, event_loop: &mut EventLoop, request: Self::Message) {
        self.process_request(request);
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if tok == BUS_TOKEN {
            self.process_signal_bus_readiness(event_loop);
        } else {
            self.process_endpoint_readiness(event_loop, tok, events);
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
    }

    fn interrupted(&mut self, _: &mut EventLoop) {
    }
}
