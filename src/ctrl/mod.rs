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

/// Requests accepted by core components
pub enum Request {
    Session(session::Request),
    Socket(socket::SocketId, socket::Request),
    Endpoint(socket::SocketId, endpoint::EndpointId, endpoint::Request)
}

/// Commands and events flowing between core and transport components.
enum Signal {
    PipeCmd(socket::SocketId, endpoint::EndpointId, PipeCmd),
    PipeEvt(socket::SocketId, endpoint::EndpointId, PipeEvt),
    AcceptorCmd(socket::SocketId, endpoint::EndpointId, AcceptorCmd),
    AcceptorEvt(socket::SocketId, endpoint::EndpointId, AcceptorEvt)
}

pub fn run_event_loop(mut event_loop: EventLoop, reply_tx: mpsc::Sender<session::Reply>) {
    let signal_bus = EventLoopBus::new();
    let signal_bus_reg = register_signal_bus(&mut event_loop, &signal_bus).expect("bus registration failed");
    let mut handler = EventLoopHandler::new(signal_bus, reply_tx);
    let exec = event_loop.run(&mut handler);

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
    fn process_session_request(&mut self, req: session::Request) {
        match req {
            session::Request::CreateSocket(ctor) => self.session.add_socket(ctor),
            _ => {}
        }
    }
    fn process_socket_request(&mut self, event_loop: &mut EventLoop, id: socket::SocketId, req: socket::Request) {
        match req {
            socket::Request::Connect(url) => self.connect(event_loop, id, url),
            socket::Request::Bind(url) => self.bind(event_loop, id, url),
            _ => {}
        }
    }

    fn connect(&mut self, event_loop: &mut EventLoop, id: socket::SocketId, url: String) {
        if let Some(socket) = self.session.get_socket_mut(id) {
            let mut ctx = SocketEventLoopContext {
                socket_id: id,
                signal_sender: &mut self.signal_bus,
                endpoints: &mut self.endpoints
            };

            socket.connect(&mut ctx, url);
        }
    }

    fn bind(&mut self, event_loop: &mut EventLoop, id: socket::SocketId, url: String) {
        if let Some(socket) = self.session.get_socket_mut(id) {
            let mut ctx = SocketEventLoopContext {
                socket_id: id,
                signal_sender: &mut self.signal_bus,
                endpoints: &mut self.endpoints
            };

            socket.bind(&mut ctx, url);
        }
    }
}

impl mio::Handler for EventLoopHandler {
    type Timeout = ();
    type Message = Request;

    fn notify(&mut self, event_loop: &mut EventLoop, request: Self::Message) {
        match request {
            Request::Session(req) => self.process_session_request(req),
            Request::Socket(id, req) => self.process_socket_request(event_loop, id, req),
            _ => {}
        }
    }

    fn ready(&mut self, event_loop: &mut EventLoop, tok: mio::Token, events: mio::EventSet) {
        println!("EventLoopHandler.ready {} {:?}", tok.0, events);
        // here find the registered io readiness observer
        // then pass and call the readiness notification method
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


struct PipeController {
    socket_id: socket::SocketId,
    endpoint_id: endpoint::EndpointId,
    pipe: Box<Endpoint<PipeCmd, PipeEvt>>
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
        Ok(From::from(0))
    }

    fn open(&mut self, endpoint_id: endpoint::EndpointId) {
        println!("SocketEventLoopContext.open {:?}", endpoint_id);
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
    signal_sender: &'a EventLoopBus<Signal>,
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
        /*let signal = EventLoopSignal::PipeEvt(self.socket_id, self.endpoint_id, evt);

        self.signal_sender.send(signal);*/
    }
}