// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::HashMap;
use std::io;

use mio::{Evented, Token, EventSet, PollOpt, EventLoop, Handler};

use core::*;
use transport::*;
use transport::tcp::Tcp;
use ctrl::bus::EventLoopBus;
use ctrl::signal::Signal;
use sequence::Sequence;
use io_error::*;

pub trait Registrar {
    fn register(&mut self, io: &Evented, tok: Token, interest: EventSet, opt: PollOpt) -> io::Result<()>;
    fn reregister(&mut self, io: &Evented, tok: Token, interest: EventSet, opt: PollOpt) -> io::Result<()>;
    fn deregister(&mut self, io: &Evented) -> io::Result<()>;
}

pub struct SocketEventLoopContext<'a> {
    socket_id: socket::SocketId,
    signal_tx: &'a mut EventLoopBus<Signal>,
    endpoints: &'a mut EndpointCollection
}

pub struct EndpointEventLoopContext<'a, 'b> {
    socket_id: socket::SocketId,
    endpoint_id: endpoint::EndpointId,
    signal_sender: &'a mut EventLoopBus<Signal>,
    registrar: &'b mut Registrar
}

pub struct PipeController {
    socket_id: socket::SocketId,
    endpoint_id: endpoint::EndpointId,
    pipe: Box<Endpoint<PipeCmd, PipeEvt>>
}

pub struct AcceptorController {
    socket_id: socket::SocketId,
    endpoint_id: endpoint::EndpointId,
    acceptor: Box<Endpoint<AcceptorCmd, AcceptorEvt>>
}

pub struct EndpointCollection {
    ids: Sequence,
    pipes: HashMap<endpoint::EndpointId, PipeController>,
    acceptors: HashMap<endpoint::EndpointId, AcceptorController>
}

impl<T:Handler> Registrar for EventLoop<T> {
    fn register(&mut self, io: &Evented, tok: Token, interest: EventSet, opt: PollOpt) -> io::Result<()> {
        self.register(io, tok, interest, opt)
    }
    fn reregister(&mut self, io: &Evented, tok: Token, interest: EventSet, opt: PollOpt) -> io::Result<()> {
        self.reregister(io, tok, interest, opt)
    }
    fn deregister(&mut self, io: &Evented) -> io::Result<()> {
        self.deregister(io)
    }
}

impl PipeController {
    pub fn ready<'a, 'b>(&mut self, registrar: &'a mut Registrar, signal_bus: &'b mut EventLoopBus<Signal>, events: EventSet) {
        let mut ctx = self.create_context(registrar, signal_bus);

        self.pipe.ready(&mut ctx, events);
    }

    pub fn process<'a, 'b>(&mut self, registrar: &'a mut Registrar, signal_bus: &'b mut EventLoopBus<Signal>, cmd: PipeCmd) {
        let mut ctx = self.create_context(registrar, signal_bus);

        self.pipe.process(&mut ctx, cmd);
    }

    fn create_context<'a, 'b>(&self, registrar: &'b mut Registrar, signal_bus: &'a mut EventLoopBus<Signal>) -> EndpointEventLoopContext<'a, 'b> {
        EndpointEventLoopContext {
            socket_id: self.socket_id,
            endpoint_id: self.endpoint_id,
            signal_sender: signal_bus,
            registrar: registrar
        }
    }
}

impl AcceptorController {
    pub fn ready<'a, 'b>(&mut self, registrar: &'a mut Registrar, signal_bus: &'b mut EventLoopBus<Signal>, events: EventSet) {
        let mut ctx = self.create_context(registrar, signal_bus);

        self.acceptor.ready(&mut ctx, events);
    }

    pub fn process<'a, 'b>(&mut self, registrar: &'a mut Registrar, signal_bus: &'b mut EventLoopBus<Signal>, cmd: AcceptorCmd) {
        let mut ctx = self.create_context(registrar, signal_bus);

        self.acceptor.process(&mut ctx, cmd);
    }

    fn create_context<'a, 'b>(&self, registrar: &'b mut Registrar, signal_bus: &'a mut EventLoopBus<Signal>) -> EndpointEventLoopContext<'a, 'b> {
        EndpointEventLoopContext {
            socket_id: self.socket_id,
            endpoint_id: self.endpoint_id,
            signal_sender: signal_bus,
            registrar: registrar
        }
    }
}
impl EndpointCollection {
    pub fn new(seq: Sequence) -> EndpointCollection {
        EndpointCollection {
            ids: seq,
            pipes: HashMap::new(),
            acceptors: HashMap::new()
        }
    }

    pub fn get_pipe_mut<'a>(&'a mut self, eid: endpoint::EndpointId) -> Option<&'a mut PipeController> {
        self.pipes.get_mut(&eid)
    }

    pub fn insert_pipe(&mut self, sid: socket::SocketId, ep: Box<Endpoint<PipeCmd, PipeEvt>>) -> endpoint::EndpointId {
        let eid = endpoint::EndpointId::from(self.ids.next());
        let controller = PipeController {
            socket_id: sid,
            endpoint_id: eid,
            pipe: ep
        };

        self.pipes.insert(eid, controller);

        eid
    }


    pub fn get_acceptor_mut<'a>(&'a mut self, eid: endpoint::EndpointId) -> Option<&'a mut AcceptorController> {
        self.acceptors.get_mut(&eid)
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

impl<'a> SocketEventLoopContext<'a> {
    pub fn new(
        sid: socket::SocketId,
        tx: &'a mut EventLoopBus<Signal>,
        eps: &'a mut EndpointCollection) -> SocketEventLoopContext<'a> {
        SocketEventLoopContext {
            socket_id: sid,
            signal_tx: tx,
            endpoints: eps,
        }
    }

    fn send_signal(&mut self, signal: Signal) {
        self.signal_tx.send(signal);
    }

    fn send_pipe_cmd(&mut self, endpoint_id: endpoint::EndpointId, cmd: PipeCmd) {
        let signal = Signal::PipeCmd(self.socket_id, endpoint_id, cmd);

        self.send_signal(signal);
    }

    fn send_acceptor_cmd(&mut self, endpoint_id: endpoint::EndpointId, cmd: AcceptorCmd) {
        let signal = Signal::AcceptorCmd(self.socket_id, endpoint_id, cmd);

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
    fn open(&mut self, endpoint_id: endpoint::EndpointId, remote: bool) {
        if remote {
            self.send_pipe_cmd(endpoint_id, PipeCmd::Open);
        } else {
            self.send_acceptor_cmd(endpoint_id, AcceptorCmd::Open)
        }
    }
    fn close(&mut self, endpoint_id: endpoint::EndpointId, remote: bool) {
        if remote {
            self.send_pipe_cmd(endpoint_id, PipeCmd::Close);
        } else {
            self.send_acceptor_cmd(endpoint_id, AcceptorCmd::Close)
        }
    }
    fn send(&mut self, endpoint_id: endpoint::EndpointId, msg: Rc<message::Message>) {
        self.send_pipe_cmd(endpoint_id, PipeCmd::Send(msg));
    }
    fn recv(&mut self, endpoint_id: endpoint::EndpointId) {
        self.send_pipe_cmd(endpoint_id, PipeCmd::Recv);
    }

}

impl<'a, 'b> Context<PipeEvt> for EndpointEventLoopContext<'a, 'b> {
    fn register(&mut self, io: &Evented, interest: EventSet, opt: PollOpt) -> io::Result<()> {
        self.registrar.register(io, self.endpoint_id.into(), interest, opt)
    }
    fn reregister(&mut self, io: &Evented, interest: EventSet, opt: PollOpt) -> io::Result<()> {
        self.registrar.reregister(io, self.endpoint_id.into(), interest, opt)
    }
    fn deregister(&mut self, io: &Evented) -> io::Result<()> {
        self.registrar.deregister(io)
    }
    fn raise(&mut self, evt: PipeEvt) {
        let signal = Signal::PipeEvt(self.socket_id, self.endpoint_id, evt);

        self.signal_sender.send(signal);
    }
}

impl<'a, 'b> Context<AcceptorEvt> for EndpointEventLoopContext<'a, 'b> {
    fn register(&mut self, io: &Evented, interest: EventSet, opt: PollOpt) -> io::Result<()> {
        self.registrar.register(io, self.endpoint_id.into(), interest, opt)
    }
    fn reregister(&mut self, io: &Evented, interest: EventSet, opt: PollOpt) -> io::Result<()> {
        self.registrar.reregister(io, self.endpoint_id.into(), interest, opt)
    }
    fn deregister(&mut self, io: &Evented) -> io::Result<()> {
        self.registrar.deregister(io)
    }
    fn raise(&mut self, evt: AcceptorEvt) {
        let signal = Signal::AcceptorEvt(self.socket_id, self.endpoint_id, evt);

        self.signal_sender.send(signal);
    }
}

impl Into<Token> for endpoint::EndpointId {
    fn into(self) -> Token {
        Token(self.into())
    }
}

impl<'x> Into<Token> for &'x endpoint::EndpointId {
    fn into(self) -> Token {
        Token(self.into())
    }
}

impl From<Token> for endpoint::EndpointId {
    fn from(tok: Token) -> endpoint::EndpointId {
        endpoint::EndpointId::from(tok.0)
    }
}
