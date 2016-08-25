// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

mod bus;

use std::collections::HashMap;
use std::rc::Rc;
use std::io;
use std::thread;
use std::sync::mpsc;
use std::time;

use mio;

use core::session;
use core::socket;
use core::endpoint;
use core::network;
use transport::*;
use transport::tcp::*;
use io_error::*;
use sequence::Sequence;
use message::Message;
use self::bus::EventLoopBus;

// Remove this when discarding mio's event_loop
const BUS_TOKEN: mio::Token = mio::Token(::std::usize::MAX - 3);

#[doc(hidden)]
pub type EventLoop = mio::EventLoop<EventLoopHandler>;

/// Requests flowing to core components via the controller
pub enum Request {
    Session(session::Request),
    Socket(socket::SocketId, socket::Request),
    Endpoint(socket::SocketId, endpoint::EndpointId, endpoint::Request)
}

/// Commands and events flowing between the controller and transport components.
enum Signal {
    PipeCmd(socket::SocketId, endpoint::EndpointId, PipeCmd),
    PipeEvt(socket::SocketId, endpoint::EndpointId, PipeEvt),
    AcceptorCmd(socket::SocketId, endpoint::EndpointId, AcceptorCmd),
    AcceptorEvt(socket::SocketId, endpoint::EndpointId, AcceptorEvt)
}

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
    fn process_socket_request(&mut self, event_loop: &mut EventLoop, id: socket::SocketId, request: socket::Request) {
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
            let mut ctx = SocketEventLoopContext {
                socket_id: id,
                signal_sender: &mut self.signal_bus,
                endpoints: &mut self.endpoints
            };

            f(socket, &mut ctx);
        }
    }
    fn process_pipe_cmd(&mut self, event_loop: &mut EventLoop, sid: socket::SocketId, eid: endpoint::EndpointId, cmd: PipeCmd) {
        if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
            pipe.process(event_loop, &mut self.signal_bus, cmd);
        }
    }
    fn process_pipe_evt(&mut self, event_loop: &mut EventLoop, sid: socket::SocketId, eid: endpoint::EndpointId, evt: PipeEvt) {
        match evt {
            PipeEvt::Opened => self.apply_on_socket(sid, |socket, ctx| socket.on_pipe_opened(ctx, eid)),
            _ => {}
        }
        println!("process_pipe_evt {:?} {:?}", sid, eid);
    }
    fn process_acceptor_cmd(&mut self, event_loop: &mut EventLoop, sid: socket::SocketId, eid: endpoint::EndpointId, cmd: AcceptorCmd) {
        println!("process_acceptor_cmd {:?} {:?}", sid, eid);
    }
    fn process_acceptor_evt(&mut self, event_loop: &mut EventLoop, sid: socket::SocketId, eid: endpoint::EndpointId, cmd: AcceptorEvt) {
        println!("process_acceptor_evt {:?} {:?}", sid, eid);
    }
    fn process_endpoint_readiness(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        let eid = endpoint::EndpointId::from(tok);
        if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
            pipe.ready(event_loop, &mut self.signal_bus, events);
        }
    }
    fn process_signal_bus_readiness(&mut self, event_loop: &mut EventLoop) {
        while let Some(signal) = self.signal_bus.recv() {
            match signal {
                Signal::PipeCmd(sid, eid, cmd) => self.process_pipe_cmd(event_loop, sid, eid, cmd),
                Signal::PipeEvt(sid, eid, cmd) => self.process_pipe_evt(event_loop, sid, eid, cmd),
                Signal::AcceptorCmd(sid, eid, cmd) => self.process_acceptor_cmd(event_loop, sid, eid, cmd),
                Signal::AcceptorEvt(sid, eid, cmd) => self.process_acceptor_evt(event_loop, sid, eid, cmd)
            }
        }
    }
}

impl mio::Handler for EventLoopHandler {
    type Timeout = ();
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
    }

    fn interrupted(&mut self, _: &mut EventLoop) {
    }
}

impl Into<mio::Token> for endpoint::EndpointId {
    fn into(self) -> mio::Token {
        mio::Token(self.into())
    }
}

impl<'x> Into<mio::Token> for &'x endpoint::EndpointId {
    fn into(self) -> mio::Token {
        mio::Token(self.into())
    }
}

impl From<mio::Token> for endpoint::EndpointId {
    fn from(tok: mio::Token) -> endpoint::EndpointId {
        endpoint::EndpointId::from(tok.0)
    }
}

struct PipeController {
    socket_id: socket::SocketId,
    endpoint_id: endpoint::EndpointId,
    pipe: Box<Endpoint<PipeCmd, PipeEvt>>
}

impl PipeController {
    fn ready(&mut self, event_loop: &mut EventLoop, signal_bus: &mut EventLoopBus<Signal>, events: mio::EventSet) {
        let mut ctx = EndpointEventLoopContext {
            socket_id: self.socket_id,
            endpoint_id: self.endpoint_id,
            signal_sender: signal_bus,
            event_loop: event_loop
        };

        self.pipe.ready(&mut ctx, events);
    }

    fn process(&mut self, event_loop: &mut EventLoop, signal_bus: &mut EventLoopBus<Signal>, cmd: PipeCmd) {
        let mut ctx = EndpointEventLoopContext {
            socket_id: self.socket_id,
            endpoint_id: self.endpoint_id,
            signal_sender: signal_bus,
            event_loop: event_loop
        };

        self.pipe.process(&mut ctx, cmd);
    }
}

struct AcceptorController {
    socket_id: socket::SocketId,
    endpoint_id: endpoint::EndpointId,
    acceptor: Box<Endpoint<AcceptorCmd, AcceptorEvt>>
}

struct EndpointCollection {
    ids: Sequence,
    pipes: HashMap<endpoint::EndpointId, PipeController>,
    acceptors: HashMap<endpoint::EndpointId, AcceptorController>
}

impl EndpointCollection {
    fn new(seq: Sequence) -> EndpointCollection {
        EndpointCollection {
            ids: seq,
            pipes: HashMap::new(),
            acceptors: HashMap::new()
        }
    }

    fn get_pipe_mut<'a>(&'a mut self, eid: endpoint::EndpointId) -> Option<&'a mut PipeController> {
        self.pipes.get_mut(&eid)
    }

    fn insert_pipe(&mut self, sid: socket::SocketId, ep: Box<Endpoint<PipeCmd, PipeEvt>>) -> endpoint::EndpointId {
        let eid = endpoint::EndpointId::from(self.ids.next());
        let controller = PipeController {
            socket_id: sid,
            endpoint_id: eid,
            pipe: ep
        };

        self.pipes.insert(eid, controller);

        eid
    }

    fn insert_acceptor(&mut self, sid: socket::SocketId, ep: Box<Endpoint<AcceptorCmd, AcceptorEvt>>) -> endpoint::EndpointId {
        let eid = endpoint::EndpointId::from(self.ids.next());
        let controller = AcceptorController {
            socket_id: sid,
            endpoint_id: eid,
            acceptor: ep
        };

        self.acceptors.insert(eid, controller);

        eid
    }
}

struct SocketEventLoopContext<'a> {
    socket_id: socket::SocketId,
    signal_sender: &'a mut EventLoopBus<Signal>,
    endpoints: &'a mut EndpointCollection
}

impl<'a> SocketEventLoopContext<'a> {
    fn send_signal(&mut self, signal: Signal) {
        self.signal_sender.send(signal);
    }

    fn send_pipe_cmd(&mut self, endpoint_id: endpoint::EndpointId, cmd: PipeCmd) {
        let signal = Signal::PipeCmd(self.socket_id, endpoint_id, cmd);

        self.send_signal(signal);
    }

    fn get_transport(&self, scheme: &str) -> io::Result<Box<Transport>> {
        match scheme {
            "tcp" => Ok(Box::new(Tcp)),
            _ => Err(invalid_input_io_error(scheme.to_owned()))
        }
    }
}

impl<'a> network::Network for SocketEventLoopContext<'a> {

    fn connect(&mut self, socket_id: socket::SocketId, url: &str, pids: (u16, u16)) -> io::Result<endpoint::EndpointId> {
        let index = match url.find("://") {
            Some(x) => x,
            None => return Err(invalid_input_io_error(url.to_owned()))
        };

        let (scheme, remainder) = url.split_at(index);
        let addr = &remainder[3..];
        let transport = try!(self.get_transport(scheme));
        let endpoint = try!(transport.connect(addr, pids));
        let id = self.endpoints.insert_pipe(socket_id, endpoint);

        Ok(id)
    }

    fn bind(&mut self, socket_id: socket::SocketId, url: &str, pids: (u16, u16)) -> io::Result<endpoint::EndpointId> {
        let index = match url.find("://") {
            Some(x) => x,
            None => return Err(invalid_input_io_error(url.to_owned()))
        };

        let (scheme, remainder) = url.split_at(index);
        let addr = &remainder[3..];
        let transport = try!(self.get_transport(scheme));
        let endpoint = try!(transport.bind(addr, pids));
        let id = self.endpoints.insert_acceptor(socket_id, endpoint);

        Ok(id)
    }

    fn open(&mut self, endpoint_id: endpoint::EndpointId) {
        self.send_pipe_cmd(endpoint_id, PipeCmd::Open);
    }
    fn close(&mut self, endpoint_id: endpoint::EndpointId) {
        self.send_pipe_cmd(endpoint_id, PipeCmd::Close);
    }
    fn send(&mut self, endpoint_id: endpoint::EndpointId, msg: Rc<Message>) {
        self.send_pipe_cmd(endpoint_id, PipeCmd::Send(msg));
    }
    fn recv(&mut self, endpoint_id: endpoint::EndpointId) {
        self.send_pipe_cmd(endpoint_id, PipeCmd::Recv);
    }
}

struct EndpointEventLoopContext<'a, 'b> {
    socket_id: socket::SocketId,
    endpoint_id: endpoint::EndpointId,
    signal_sender: &'a mut EventLoopBus<Signal>,
    event_loop: &'b mut EventLoop
}

impl<'a, 'b> Registrar for EndpointEventLoopContext<'a, 'b> {
    fn register(&mut self, io: &mio::Evented, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()> {
        self.event_loop.register(io, self.endpoint_id.into(), interest, opt)
    }

    fn reregister(&mut self, io: &mio::Evented, interest: mio::EventSet, opt: mio::PollOpt) -> io::Result<()> {
        self.event_loop.reregister(io, self.endpoint_id.into(), interest, opt)
    }
    fn deregister(&mut self, io: &mio::Evented) -> io::Result<()> {
        self.event_loop.deregister(io)
    }
}

impl<'a, 'b> Context<PipeEvt> for EndpointEventLoopContext<'a, 'b> {
    fn raise(&mut self, evt: PipeEvt) {
        let signal = Signal::PipeEvt(self.socket_id, self.endpoint_id, evt);

        self.signal_sender.send(signal);
    }
}