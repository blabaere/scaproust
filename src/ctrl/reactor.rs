// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;

use mio;

use core::{SocketId, EndpointId, session, socket, endpoint, context};

use transport::pipe;
use transport::acceptor;

use ctrl::Signal;
use ctrl::bus::EventLoopBus;
use ctrl::adapter::{
    EndpointCollection,
    Schedule,
    SocketEventLoopContext
};
use sequence::Sequence;

/// Requests flowing to core components via the controller
pub enum Request {
    Session(session::Request),
    Socket(SocketId, socket::Request),
    Endpoint(SocketId, EndpointId, endpoint::Request)
}

const BUS_TOKEN: mio::Token = mio::Token(::std::usize::MAX - 3);

#[doc(hidden)]
pub type EventLoop = mio::EventLoop<Reactor>;

pub fn run_event_loop(mut event_loop: EventLoop, reply_tx: mpsc::Sender<session::Reply>) {
    let signal_bus = EventLoopBus::new();
    let signal_bus_reg = register_signal_bus(&mut event_loop, &signal_bus);
    let mut handler = Reactor::new(signal_bus, reply_tx);
    let exec = signal_bus_reg.and_then(|_| event_loop.run(&mut handler));

    match exec {
        Ok(_) => debug!("event loop exited"),
        Err(e) => error!("event loop failed to run: {}", e)
    }
}

fn register_signal_bus(event_loop: &mut EventLoop, signal_bus: &EventLoopBus<Signal>) -> io::Result<()> {
    event_loop.register(signal_bus, BUS_TOKEN, mio::EventSet::readable(), mio::PollOpt::edge())
}

pub struct Reactor {
    signal_bus: EventLoopBus<Signal>,
    session: session::Session,
    endpoints: EndpointCollection,
    schedule: Schedule
}

impl Reactor {
    fn new(bus: EventLoopBus<Signal>, reply_tx: mpsc::Sender<session::Reply>) -> Reactor {
        let seq = Sequence::new();
        Reactor {
            signal_bus: bus,
            session: session::Session::new(seq.clone(), reply_tx),
            endpoints: EndpointCollection::new(seq.clone()),
            schedule: Schedule::new(seq.clone())
        }
    }
    fn process_request(&mut self, event_loop: &mut EventLoop, request: Request) {
        match request {
            Request::Session(req) => self.process_session_request(req),
            Request::Socket(id, req) => self.process_socket_request(event_loop, id, req),
            _ => {}
        }
    }
    fn process_session_request(&mut self, request: session::Request) {
        match request {
            session::Request::CreateSocket(ctor) => self.session.add_socket(ctor),
            _ => {}
        }
    }
    fn process_socket_request(&mut self, event_loop: &mut EventLoop, id: SocketId, request: socket::Request) {
        match request {
            socket::Request::Connect(url) => self.apply_on_socket(event_loop, id, |socket, ctx| socket.connect(ctx, url)),
            socket::Request::Bind(url)    => self.apply_on_socket(event_loop, id, |socket, ctx| socket.bind(ctx, url)),
            socket::Request::Send(msg)    => self.apply_on_socket(event_loop, id, |socket, ctx| socket.send(ctx, msg)),
            socket::Request::Recv         => self.apply_on_socket(event_loop, id, |socket, ctx| socket.recv(ctx)),
            socket::Request::SetOption(x) => self.apply_on_socket(event_loop, id, |socket, ctx| socket.set_opt(ctx, x))
        }
    }
    fn process_timeout(&mut self, event_loop: &mut EventLoop, task: context::Schedulable) {
        match task {
            context::Schedulable::Reconnect(sid, eid, url) => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.reconnect(ctx, url, eid)),
            context::Schedulable::Rebind(sid, eid, url)    => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.rebind(ctx, url, eid)),
            context::Schedulable::SendTimeout(sid)         => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.on_send_timeout(ctx)),
            _ => {},
        }

    }
    fn process_endpoint_readiness(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        let eid = EndpointId::from(tok);
        {
            if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
                pipe.ready(event_loop, &mut self.signal_bus, events);
                return;
            } 
        }
        {
            if let Some(acceptor) = self.endpoints.get_acceptor_mut(eid) {
                acceptor.ready(event_loop, &mut self.signal_bus, events);
                return;
            }
        }
    }
    fn process_signal_bus_readiness(&mut self, event_loop: &mut EventLoop) {
        while let Some(signal) = self.signal_bus.recv() {
            self.process_signal(event_loop, signal);
        }
    }
    fn process_signal(&mut self, event_loop: &mut EventLoop, signal: Signal) {
        match signal {
            Signal::PipeCmd(_, eid, cmd)       => self.process_pipe_cmd(event_loop, eid, cmd),
            Signal::AcceptorCmd(_, eid, cmd)   => self.process_acceptor_cmd(event_loop, eid, cmd),
            Signal::PipeEvt(sid, eid, evt)     => self.process_pipe_evt(event_loop, sid, eid, evt),
            Signal::AcceptorEvt(sid, eid, evt) => self.process_acceptor_evt(event_loop, sid, eid, evt),
            Signal::SocketEvt(_, _) => {}
        }
    }
    fn process_pipe_cmd(&mut self, event_loop: &mut EventLoop, eid: EndpointId, cmd: pipe::Command) {
        if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
            pipe.process(event_loop, &mut self.signal_bus, cmd);
        }
    }
    fn process_acceptor_cmd(&mut self, event_loop: &mut EventLoop, eid: EndpointId, cmd: acceptor::Command) {
        if let Some(acceptor) = self.endpoints.get_acceptor_mut(eid) {
            acceptor.process(event_loop, &mut self.signal_bus, cmd);
        }
    }
    fn process_pipe_evt(&mut self, event_loop: &mut EventLoop, sid: SocketId, eid: EndpointId, evt: pipe::Event) {
        match evt {
            pipe::Event::Opened        => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.on_pipe_opened(ctx, eid)),
            pipe::Event::Sent          => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.on_send_ack(ctx, eid)),
            pipe::Event::Received(msg) => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.on_recv_ack(ctx, eid, msg)),
            pipe::Event::Error(err)    => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.on_pipe_error(ctx, eid, err)),
            pipe::Event::Closed        => self.endpoints.remove_pipe(eid),
            _ => {}
        }
    }
    fn process_acceptor_evt(&mut self, event_loop: &mut EventLoop, sid: SocketId, eid: EndpointId, evt: acceptor::Event) {
        match evt {
            // Maybe the controller should be removed from the endpoint collection
            acceptor::Event::Error(e) => self.apply_on_socket(event_loop, sid, |socket, ctx| socket.on_acceptor_error(ctx, eid, e)),
            acceptor::Event::Accepted(pipes) => {
                for pipe in pipes {
                    let pipe_id = self.endpoints.insert_pipe(sid, pipe);

                    self.apply_on_socket(event_loop, sid, |socket, ctx| socket.on_pipe_accepted(ctx, pipe_id));
                }
            },
            _ => {}
        }
    }
    fn apply_on_socket<F>(&mut self, event_loop: &mut EventLoop, id: SocketId, f: F) 
    where F : FnOnce(&mut socket::Socket, &mut SocketEventLoopContext) {
        if let Some(socket) = self.session.get_socket_mut(id) {
            let mut ctx = SocketEventLoopContext::new(id, &mut self.signal_bus, &mut self.endpoints, &mut self.schedule, event_loop);

            f(socket, &mut ctx);
        }
    }
}

impl mio::Handler for Reactor {
    type Timeout = context::Schedulable;
    type Message = Request;

    fn notify(&mut self, event_loop: &mut EventLoop, request: Self::Message) {
        self.process_request(event_loop, request);
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        if tok == BUS_TOKEN {
            self.process_signal_bus_readiness(event_loop);
        } else {
            self.process_endpoint_readiness(event_loop, tok, events);
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
        self.process_timeout(event_loop, timeout);
    }

    fn interrupted(&mut self, _: &mut EventLoop) {
    }
}
