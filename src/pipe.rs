// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use byteorder::{ BigEndian, WriteBytesExt };

use mio;

use EventLoop;
use Message;
use transport::Connection;
use global;
use event_loop_msg::*;
use send;
use recv;

// A pipe is responsible for handshaking with its peer and transfering raw messages over a connection.
// That means send/receive size prefix and then message payload
// according to the connection readiness and the requested operation progress if any
pub struct Pipe {
    addr: Option<String>,
    state: Option<Box<PipeState>>
}

impl Pipe {

    pub fn new(
        tok: mio::Token,
        addr: Option<String>,
        ids: (u16, u16),
        conn: Box<Connection>,
        sig_tx: mio::Sender<EventLoopSignal>) -> Pipe {

        let (p_id, p_peer_id) = ids;
        let state = Initial {
            body : PipeBody {
                token: tok,
                connection: conn,
                sig_sender: sig_tx
            },
            protocol_id: p_id,
            protocol_peer_id: p_peer_id
        };

        Pipe {
            addr: addr,
            state: Some(Box::new(state))
        }
    }

    fn on_state_transition<F>(&mut self, transition: F) where F : FnOnce(Box<PipeState>) -> Box<PipeState> {
        if let Some(state) = self.state.take() {
            self.state = Some(transition(state));
        }
    }

    pub fn open(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.open(event_loop));
    }

    pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        self.on_state_transition(|s: Box<PipeState>| s.ready(event_loop, events));
    }

    pub fn recv(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.recv(event_loop));
    }

    pub fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) {
        self.on_state_transition(|s: Box<PipeState>| s.send(event_loop, msg));
    }

    pub fn close(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.close(event_loop));
    }

    pub fn addr(self) -> Option<String> {
        self.addr
    }
}

struct PipeBody {
    token: mio::Token,
    connection: Box<Connection>,
    sig_sender: mio::Sender<EventLoopSignal>
}

impl PipeBody {
    fn token(&self) -> mio::Token {
        self.token
    }

    fn connection(&mut self) -> &mut Connection {
        &mut *self.connection
    }

    fn register(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        let interest = mio::EventSet::error() | mio::EventSet::hup();
        let poll = mio::PollOpt::edge() | mio::PollOpt::oneshot();
        let io = self.connection.as_evented();

        event_loop.register(io, self.token, interest, poll)
    }

    fn register_for_none(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.register_for_event(event_loop, mio::EventSet::none())
    }

    fn register_for_write(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.register_for_event(event_loop, mio::EventSet::writable())
    }

    fn register_for_read(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.register_for_event(event_loop, mio::EventSet::readable())
    }

    fn register_for_event(&self, event_loop: &mut EventLoop, event: mio::EventSet) -> io::Result<()> {
        let interest = mio::EventSet::error() | mio::EventSet::hup() | event;
        let poll = mio::PollOpt::edge() | mio::PollOpt::oneshot();
        let io = self.connection.as_evented();

        event_loop.reregister(io, self.token, interest, poll)
    }

    fn send_sig(&self, sig: PipeEvtSignal) -> io::Result<()> {
        let evt_sig = EvtSignal::Pipe(self.token, sig);
        let loop_sig = EventLoopSignal::Evt(evt_sig);
        let send_res = self.sig_sender.send(loop_sig);

        send_res.map_err(|e| global::convert_notify_err(e))
    }

    fn close(&self, event_loop: &mut EventLoop) -> Box<PipeState> {
        event_loop.deregister(self.connection.as_evented());
        Box::new(Dead)
    }
}

trait PipeState {
    fn open(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        // TODO test hup and error, then call readable or writable, or maybe both ?
        Box::new(Dead)
    }

    fn recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn send(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn close(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn on_error(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        // TODO send a Disconnected signal
        Box::new(Dead)
    }
}

fn transition<F, T>(f: Box<F>) -> Box<T> where
    F : PipeState,
    T : From<F>,
    T : PipeState
{
    let t: T = From::from(*f);

    Box::new(t)
}

fn transition_if_ok<F, T : 'static>(f: Box<F>, res: io::Result<()>, event_loop: &mut EventLoop) -> Box<PipeState> where
    F : PipeState,
    T : From<F>,
    T : PipeState
{
    match res {
        Ok(_) => transition::<F, T>(f),
        Err(_) => f.on_error(event_loop)
    }
}

fn no_transition_if_ok<F : PipeState + 'static>(f: Box<F>, res: io::Result<()>, event_loop: &mut EventLoop) -> Box<PipeState> 
{
    match res {
        Ok(_) => f,
        Err(_) => f.on_error(event_loop)
    }
}

struct Initial {
    body: PipeBody, 
    protocol_id: u16,
    protocol_peer_id: u16
}

impl Initial {
    fn register_for_write(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register(event_loop).and_then(|_| self.body.register_for_write(event_loop))
    }
}

impl PipeState for Initial {
    fn open(mut self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let registered = self.register_for_write(event_loop);

        transition_if_ok::<Initial, HandshakeTx>(self, registered, event_loop)
    }

    fn recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        self
    }

    fn send(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        self
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.body.close(event_loop)
    }
}

struct HandshakeTx {
    body: PipeBody, 
    protocol_id: u16,
    protocol_peer_id: u16
}

impl From<Initial> for HandshakeTx {
    fn from(state: Initial) -> HandshakeTx {
        HandshakeTx {
            body: state.body,
            protocol_id: state.protocol_id,
            protocol_peer_id: state.protocol_peer_id
        }
    }
}

impl HandshakeTx {

    fn write_handshake(&mut self) -> io::Result<()> {
        // handshake is Zero, 'S', 'P', Version, Proto, Rsvd
        let mut handshake = vec!(0, 83, 80, 0);
        try!(handshake.write_u16::<BigEndian>(self.protocol_id));
        try!(handshake.write_u16::<BigEndian>(0));
        try!(
            self.body.connection().try_write(&handshake).
            and_then(|w| self.check_sent_handshake(w)));
        debug!("[{:?}] handshake sent.", self.body.token());
        Ok(())
    }

    fn check_sent_handshake(&self, written: Option<usize>) -> io::Result<()> {
        match written {
            Some(8) => Ok(()),
            Some(_) => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send full handshake")),
            _       => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send handshake"))
        }
    }

    fn register_for_write(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_write(event_loop)
    }

    fn register_for_read(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_read(event_loop)
    }
}

impl PipeState for HandshakeTx {
    fn ready(mut self: Box<Self>, event_loop: &mut EventLoop, events: mio::EventSet) -> Box<PipeState> {
        if events.is_writable() {
            let res = self.write_handshake().and_then(|_| self.register_for_read(event_loop));

            transition_if_ok::<HandshakeTx, HandshakeRx>(self, res, event_loop)
        } else {
            let res = self.register_for_write(event_loop);

            no_transition_if_ok::<HandshakeTx>(self, res, event_loop)
        }
    }

    fn recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        self
    }

    fn send(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        self
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.body.close(event_loop)
    }
}

struct HandshakeRx {
    body: PipeBody, 
    protocol_peer_id: u16
}

impl From<HandshakeTx> for HandshakeRx {
    fn from(state: HandshakeTx) -> HandshakeRx {
        HandshakeRx {
            body: state.body,
            protocol_peer_id: state.protocol_peer_id
        }
    }
}

impl HandshakeRx {

    fn register_for_none(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_none(event_loop)
    }

    fn register_for_read(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_read(event_loop)
    }

    fn read_handshake(&mut self) -> io::Result<()> {
        let mut handshake = [0u8; 8];
        try!(
            self.body.connection().try_read(&mut handshake).
            and_then(|_| self.check_received_handshake(&handshake)));
        debug!("[{:?}] handshake received.", self.body.token());
        Ok(())
    }

    fn check_received_handshake(&self, handshake: &[u8; 8]) -> io::Result<()> {
        let mut expected_handshake = vec!(0, 83, 80, 0);
        try!(expected_handshake.write_u16::<BigEndian>(self.protocol_peer_id));
        try!(expected_handshake.write_u16::<BigEndian>(0));
        let mut both = handshake.iter().zip(expected_handshake.iter());

        if both.all(|(l,r)| l == r) {
            Ok(())
        } else {
            error!("expected '{:?}' but received '{:?}' !", expected_handshake, handshake);
            Err(io::Error::new(io::ErrorKind::InvalidData, "received bad handshake"))
        }
    }
}

impl PipeState for HandshakeRx {
    fn ready(mut self: Box<Self>, event_loop: &mut EventLoop, events: mio::EventSet) -> Box<PipeState> {
        if events.is_readable() {
            let res = self.read_handshake().and_then(|_| self.register_for_none(event_loop));

            transition_if_ok::<HandshakeRx, Idle>(self, res, event_loop)
        } else {
            let res = self.register_for_read(event_loop);

            no_transition_if_ok::<HandshakeRx>(self, res, event_loop)
        }
    }

    fn recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        self
    }

    fn send(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        self
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.body.close(event_loop)
    }
}

struct Idle {
    body: PipeBody
}

impl From<HandshakeRx> for Idle {
    fn from(state: HandshakeRx) -> Idle {
        Idle {
            body: state.body
        }
    }
}

impl From<Receiving> for Idle {
    fn from(state: Receiving) -> Idle {
        Idle {
            body: state.body
        }
    }
}

impl From<Sending> for Idle {
    fn from(state: Sending) -> Idle {
        Idle {
            body: state.body
        }
    }
}

impl Idle {
    fn register_for_read(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_read(event_loop)
    }

    fn register_for_write(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_write(event_loop)
    }

    fn received_msg(self: Box<Self>, event_loop: &mut EventLoop, msg: Message) -> Box<PipeState> {
        let res = self.body.send_sig(PipeEvtSignal::MsgRcv(msg));

        no_transition_if_ok(self, res, event_loop)
    }

    fn receiving_msg(mut self: Box<Self>, event_loop: &mut EventLoop, op: recv::RecvOperation) -> Box<PipeState> {
        let res = self.register_for_read(event_loop);

        if res.is_ok() {
            Box::new(Receiving::from(*self, op))
        } else {
            self.on_error(event_loop)
        }
    }

    fn sent_msg(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.body.send_sig(PipeEvtSignal::MsgSnd);

        no_transition_if_ok(self, res, event_loop)
    }

    fn sending_msg(mut self: Box<Self>, event_loop: &mut EventLoop, op: send::SendOperation) -> Box<PipeState> {
        match self.register_for_write(event_loop) {
            Ok(..) => Box::new(Sending::from(*self, op)),
            Err(_) => self.on_error(event_loop),
        }
    }
}

impl PipeState for Idle {

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        self
    }

    fn recv(mut self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let mut operation = recv::RecvOperation::new();

        match operation.recv(self.body.connection()) {
            Ok(Some(msg)) => self.received_msg(event_loop, msg),
            Ok(None)      => self.receiving_msg(event_loop, operation),
            Err(_)        => self.on_error(event_loop)
        }
    }

    fn send(mut self: Box<Self>, event_loop: &mut EventLoop, msg: Rc<Message>) -> Box<PipeState> {
        match send::SendOperation::new(msg) {
            Ok(mut operation) => {
                match operation.send(self.body.connection()) {
                    Ok(true)  => self.sent_msg(event_loop),
                    Ok(false) => self.sending_msg(event_loop, operation),
                    Err(_)    => self.on_error(event_loop)
                }
            },
            Err(_) => self.on_error(event_loop),
        }
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.body.close(event_loop)
    }
}

struct Receiving {
    body: PipeBody,
    operation: Option<recv::RecvOperation>
}

impl Receiving {
    fn from(state: Idle, op: recv::RecvOperation) -> Receiving {
        Receiving {
            body: state.body,
            operation: Some(op)
        }
    }

    fn register_for_read(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_read(event_loop)
    }

    fn received_msg(self: Box<Self>, event_loop: &mut EventLoop, msg: Message) -> Box<PipeState> {
        let res = self.body.send_sig(PipeEvtSignal::MsgRcv(msg));

        transition_if_ok::<Receiving, Idle>(self, res, event_loop)
    }

    fn receiving_msg(mut self: Box<Self>, event_loop: &mut EventLoop, op: recv::RecvOperation) -> Box<PipeState> {
        let res = self.register_for_read(event_loop);

        self.operation = Some(op);

        no_transition_if_ok(self, res, event_loop)
    }
}

impl PipeState for Receiving {

    fn ready(mut self: Box<Self>, event_loop: &mut EventLoop, events: mio::EventSet) -> Box<PipeState> {

        if self.operation.is_none() {
            return Box::new(Idle { body: self.body });
        }

        if events.is_readable() == false {
            return self;
        }

        let mut operation = self.operation.take().unwrap();
        match operation.recv(self.body.connection()) {
            Ok(Some(msg)) => self.received_msg(event_loop, msg),
            Ok(None)      => self.receiving_msg(event_loop, operation),
            Err(_)        => self.on_error(event_loop)
        }
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.body.close(event_loop)
    }
}

struct Sending {
    body: PipeBody,
    operation: Option<send::SendOperation>
}

impl Sending {
    fn from(state: Idle, op: send::SendOperation) -> Sending {
        Sending {
            body: state.body,
            operation: Some(op)
        }
    }

    fn register_for_write(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.register_for_write(event_loop)
    }

    fn sent_msg(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.body.send_sig(PipeEvtSignal::MsgSnd);

        transition_if_ok::<Sending, Idle>(self, res, event_loop)
    }

    fn sending_msg(mut self: Box<Self>, event_loop: &mut EventLoop, op: send::SendOperation) -> Box<PipeState> {
        let res = self.register_for_write(event_loop);

        self.operation = Some(op);

        no_transition_if_ok(self, res, event_loop)
    }
}

impl PipeState for Sending {

    fn ready(mut self: Box<Self>, event_loop: &mut EventLoop, events: mio::EventSet) -> Box<PipeState> {

        if self.operation.is_none() {
            return Box::new(Idle { body: self.body });
        }

        if events.is_writable() == false {
            return self;
        }

        let mut operation = self.operation.take().unwrap();
        match operation.send(self.body.connection()) {
                Ok(true)  => self.sent_msg(event_loop),
                Ok(false) => self.sending_msg(event_loop, operation),
                Err(_)    => self.on_error(event_loop)
        }
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.body.close(event_loop)
    }
}

struct Dead;

impl PipeState for Dead {
}
