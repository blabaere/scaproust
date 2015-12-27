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

// TODO ? : split recv & send related state so that a pipe can be both sending and receiving
// this could be usefulfor req resend where the pipe could be receiving 
// and then ask to send (the same request)

// A pipe is responsible for handshaking with its peer and transfering raw messages over a connection.
// That means send/receive size prefix and then message payload
// according to the connection readiness and the requested operation progress if any
pub struct Pipe {
    token: mio::Token,
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
            token: tok,
            addr: addr,
            state: Some(Box::new(state))
        }
    }

    fn on_state_transition<F>(&mut self, transition: F) where F : FnOnce(Box<PipeState>) -> Box<PipeState> {
        if let Some(old_state) = self.state.take() {
            let old_name = old_state.name();
            let new_state = transition(old_state);
            let new_name = new_state.name();

            self.state = Some(new_state);

            debug!("[{:?}] switch from '{}' to '{}'.", self.token, old_name, new_name);
        }
    }

    pub fn register(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.register(event_loop));
    }

    pub fn on_register(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.on_register(event_loop));
    }

    pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        self.on_state_transition(|s: Box<PipeState>| s.ready(event_loop, events));
    }

    pub fn recv(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.recv(event_loop));
    }

    pub fn cancel_recv(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.cancel_recv(event_loop));
    }

    pub fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) {
        self.on_state_transition(|s: Box<PipeState>| s.send(event_loop, msg));
    }

    pub fn send_nb(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) {
        self.on_state_transition(|s: Box<PipeState>| s.send_nb(event_loop, msg));
    }

    pub fn cancel_send(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.cancel_send(event_loop));
    }

    pub fn unregister(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(|s: Box<PipeState>| s.unregister(event_loop));
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

    fn register(&self,
        event_loop: &mut EventLoop, 
        events: mio::EventSet,
        opt: mio::PollOpt) -> io::Result<()> {
        let io = self.connection.as_evented();

        event_loop.register(io, self.token, events, opt)
    }

    fn reregister(&self,
        event_loop: &mut EventLoop, 
        events: mio::EventSet,
        opt: mio::PollOpt) -> io::Result<()> {
        let io = self.connection.as_evented();

        event_loop.reregister(io, self.token, mio::EventSet::hup() | mio::EventSet::error() | events, opt)
    }

    fn send_sig(&self, sig: PipeEvtSignal) -> io::Result<()> {
        let evt_sig = EvtSignal::Pipe(self.token, sig);
        let loop_sig = EventLoopSignal::Evt(evt_sig);
        let send_res = self.sig_sender.send(loop_sig);

        send_res.map_err(|e| global::convert_notify_err(e))
    }

    fn unregister(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        event_loop.deregister(self.connection.as_evented())
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// STATE DEFINITION                                                          //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
trait PipeState {
    fn name(&self) -> &'static str;

    fn register(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn on_register(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        // TODO test hup and error, then call readable or writable, or maybe both ?
        Box::new(Dead)
    }

    fn recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn cancel_recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn send(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn send_nb(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn cancel_send(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn unregister(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        Box::new(Dead)
    }

    fn on_error(self: Box<Self>, _: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        debug!("State '{}' failed: {:?}", self.name(), err);
        // TODO send a Disconnected signal
        Box::new(Dead)
    }
}

trait LivePipeState {
    fn body<'a>(&'a self) -> &'a PipeBody;

    fn unregister(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body().unregister(event_loop)
    }

    fn debug(&self, log: &str) {
        debug!("[{:?}] {}", self.body().token(), log)
    }

    fn send_sig(&self, sig: PipeEvtSignal) -> io::Result<()> {
        self.body().send_sig(sig)
    }
}

fn unregister_live(state: &LivePipeState, event_loop: &mut EventLoop) -> Box<PipeState> {
    state.unregister(event_loop);
    Box::new(Dead)
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
        Err(e) => f.on_error(event_loop, e)
    }
}

fn no_transition_if_ok<F : PipeState + 'static>(f: Box<F>, res: io::Result<()>, event_loop: &mut EventLoop) -> Box<PipeState> 
{
    match res {
        Ok(_) => f,
        Err(e) => f.on_error(event_loop, e)
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// INITIAL STATE                                                             //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
struct Initial {
    body: PipeBody, 
    protocol_id: u16,
    protocol_peer_id: u16
}

impl PipeState for Initial {
    fn name(&self) -> &'static str {
        "Initial"
    }

    fn register(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.body.register(event_loop, mio::EventSet::writable(), mio::PollOpt::level());

        transition_if_ok::<Initial, HandshakeTx>(self, res, event_loop)
    }

    fn unregister(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        unregister_live(self.as_ref(), event_loop)
    }
}

impl LivePipeState for Initial {
    fn body<'a>(&'a self) -> &'a PipeBody {
        &self.body
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// SEND HANDSHAKE STATE                                                      //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
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
        self.debug("handshake sent.");
        Ok(())
    }

    fn check_sent_handshake(&self, written: Option<usize>) -> io::Result<()> {
        match written {
            Some(8) => Ok(()),
            Some(_) => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send full handshake")),
            _       => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send handshake"))
        }
    }

    fn subscribe_to_readable(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.reregister(event_loop, mio::EventSet::readable(), mio::PollOpt::level())
    }

    fn subscribe_to_writable(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.reregister(event_loop, mio::EventSet::writable(), mio::PollOpt::level())
    }
}

impl PipeState for HandshakeTx {
    fn name(&self) -> &'static str {
        "Send handshake"
    }

    fn ready(mut self: Box<Self>, event_loop: &mut EventLoop, events: mio::EventSet) -> Box<PipeState> {
        if events.is_writable() {
            let res = self.write_handshake().and_then(|_| self.subscribe_to_readable(event_loop));

            transition_if_ok::<HandshakeTx, HandshakeRx>(self, res, event_loop)
        } else {
            let res = self.subscribe_to_writable(event_loop);

            no_transition_if_ok::<HandshakeTx>(self, res, event_loop)
        }
    }

    fn unregister(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        unregister_live(self.as_ref(), event_loop)
    }
}

impl LivePipeState for HandshakeTx {
    fn body<'a>(&'a self) -> &'a PipeBody {
        &self.body
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// RECV PEER HANDSHAKE STATE                                                 //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
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

    fn read_handshake(&mut self) -> io::Result<()> {
        let mut handshake = [0u8; 8];
        try!(
            self.body.connection().try_read(&mut handshake).
            and_then(|_| self.check_received_handshake(&handshake)));
        self.debug("handshake received.");
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

    fn subscribe_to_readable(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.reregister(event_loop, mio::EventSet::readable(), mio::PollOpt::level())
    }

    fn send_sig(&self, sig: PipeEvtSignal) -> io::Result<()> {
        self.body.send_sig(sig)
    }
}

impl PipeState for HandshakeRx {
    fn name(&self) -> &'static str {
        "Recv handshake"
    }

    fn ready(mut self: Box<Self>, event_loop: &mut EventLoop, events: mio::EventSet) -> Box<PipeState> {
        if events.is_readable() {
            let res = self.read_handshake().and_then(|_| self.send_sig(PipeEvtSignal::Opened));

            transition_if_ok::<HandshakeRx, Activable>(self, res, event_loop)
        } else {
            let res = self.subscribe_to_readable(event_loop);

            no_transition_if_ok::<HandshakeRx>(self, res, event_loop)
        }
    }

    fn unregister(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        unregister_live(self.as_ref(), event_loop)
    }
}

impl LivePipeState for HandshakeRx {
    fn body<'a>(&'a self) -> &'a PipeBody {
        &self.body
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// HANDSHAKE ESTABLISHED STATE                                               //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
struct Activable {
    body: PipeBody
}

impl Activable {
    fn subscribe_to_all(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.reregister(
                event_loop, 
                mio::EventSet::readable() | mio::EventSet::writable(), 
                mio::PollOpt::edge())
        
    }
}

impl From<HandshakeRx> for Activable {
    fn from(state: HandshakeRx) -> Activable {
        Activable {
            body: state.body
        }
    }
}

impl PipeState for Activable {
    fn name(&self) -> &'static str {
        "Handshaked"
    }

    fn on_register(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.subscribe_to_all(event_loop);

        transition_if_ok::<Activable, Idle>(self, res, event_loop)
    }

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        self
    }

    fn unregister(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        unregister_live(self.as_ref(), event_loop)
    }
}

impl LivePipeState for Activable {
    fn body<'a>(&'a self) -> &'a PipeBody {
        &self.body
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// IDLE STATE                                                                //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
struct Idle {
    body: PipeBody
}

impl From<Activable> for Idle {
    fn from(state: Activable) -> Idle {
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
    fn received_msg(self: Box<Self>, event_loop: &mut EventLoop, msg: Message) -> Box<PipeState> {
        let res = self.send_sig(PipeEvtSignal::MsgRcv(msg));

        no_transition_if_ok(self, res, event_loop)
    }

    fn receiving_msg(self: Box<Self>, _: &mut EventLoop, op: recv::RecvOperation) -> Box<PipeState> {
        Box::new(Receiving::from(*self, op))
    }

    fn sent_msg(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.send_sig(PipeEvtSignal::MsgSnd);

        no_transition_if_ok(self, res, event_loop)
    }

    fn sending_msg(self: Box<Self>, _: &mut EventLoop, op: send::SendOperation) -> Box<PipeState> {
        Box::new(Sending::from(*self, op))
    }
}

impl PipeState for Idle {
    fn name(&self) -> &'static str {
        "Idle"
    }

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        self
    }

    fn recv(mut self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let mut operation = recv::RecvOperation::new();

        match operation.recv(self.body.connection()) {
            Ok(Some(msg)) => self.received_msg(event_loop, msg),
            Ok(None)      => self.receiving_msg(event_loop, operation),
            Err(e)        => self.on_error(event_loop, e)
        }
    }

    fn cancel_recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        self
    }

    fn send(mut self: Box<Self>, event_loop: &mut EventLoop, msg: Rc<Message>) -> Box<PipeState> {
        match send::SendOperation::new(msg) {
            Ok(mut operation) => {
                match operation.send(self.body.connection()) {
                    Ok(true)  => self.sent_msg(event_loop),
                    Ok(false) => self.sending_msg(event_loop, operation),
                    Err(e)    => self.on_error(event_loop, e)
                }
            },
            Err(e) => self.on_error(event_loop, e)
        }
    }

    fn send_nb(mut self: Box<Self>, event_loop: &mut EventLoop, msg: Rc<Message>) -> Box<PipeState> {
        match send::SendOperation::new(msg) {
            Ok(mut operation) => {
                match operation.send(self.body.connection()) {
                    Ok(true)  => self,
                    Ok(false) => self.on_error(event_loop, global::would_block_io_error("Non blocking send requested, but would block.")),
                    Err(e)    => self.on_error(event_loop, e)
                }
            },
            Err(e) => self.on_error(event_loop, e)
        }
    }

    fn cancel_send(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        self
    }

    fn unregister(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        unregister_live(self.as_ref(), event_loop)
    }
}

impl LivePipeState for Idle {
    fn body<'a>(&'a self) -> &'a PipeBody {
        &self.body
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// RECV IN PROGRESS STATE                                                    //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
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

    fn received_msg(self: Box<Self>, event_loop: &mut EventLoop, msg: Message) -> Box<PipeState> {
        let res = self.send_sig(PipeEvtSignal::MsgRcv(msg));

        transition_if_ok::<Receiving, Idle>(self, res, event_loop)
    }

    fn receiving_msg(mut self: Box<Self>, _: &mut EventLoop, op: recv::RecvOperation) -> Box<PipeState> {
        self.operation = Some(op);
        self
    }
}

impl PipeState for Receiving {
    fn name(&self) -> &'static str {
        "Recv"
    }

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
            Err(e)        => self.on_error(event_loop, e)
        }
    }

    fn cancel_recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        transition::<Receiving, Idle>(self)
    }
    fn unregister(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        unregister_live(self.as_ref(), event_loop)
    }
}

impl LivePipeState for Receiving {
    fn body<'a>(&'a self) -> &'a PipeBody {
        &self.body
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// SEND IN PROGRESS STATE                                                    //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
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

    fn sent_msg(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.send_sig(PipeEvtSignal::MsgSnd);

        transition_if_ok::<Sending, Idle>(self, res, event_loop)
    }

    fn sending_msg(mut self: Box<Self>, _: &mut EventLoop, op: send::SendOperation) -> Box<PipeState> {
        self.operation = Some(op);
        self
    }
}

impl PipeState for Sending {
    fn name(&self) -> &'static str {
        "Send"
    }

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
                Err(e)    => self.on_error(event_loop, e)
        }
    }

    fn cancel_send(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        transition::<Sending, Idle>(self)
    }

    fn unregister(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        unregister_live(self.as_ref(), event_loop)
    }
}

impl LivePipeState for Sending {
    fn body<'a>(&'a self) -> &'a PipeBody {
        &self.body
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// DEAD STATE                                                                //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
struct Dead;

impl PipeState for Dead {
    fn name(&self) -> &'static str {
        "Dead"
    }
}
