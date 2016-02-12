// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use byteorder::*;

use mio;

use EventLoop;
use Message;
use transport::Connection;
use global;
use event_loop_msg::*;
use send;
use recv;

/// A pipe is responsible for handshaking with its peer and transfering raw messages over a connection.
/// That means send/receive size prefix and then message payload.
/// This is done according to the connection readiness and the operation progress.
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
            state: Some(box state)
        }
    }

    fn apply<F>(&mut self, transition: F) where F : FnOnce(Box<PipeState>) -> Box<PipeState> {
        if let Some(old_state) = self.state.take() {
            let old_name = old_state.name();
            let new_state = transition(old_state);
            let new_name = new_state.name();

            self.state = Some(new_state);

            debug!("[{:?}] switch from '{}' to '{}'.", self.token.as_usize(), old_name, new_name);
        }
    }

    pub fn open(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.open(event_loop));
    }

    pub fn on_open_ack(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.on_open_ack(event_loop));
    }

    pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        self.apply(|s| s.ready(event_loop, events));
    }

    pub fn recv(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.recv(event_loop));
    }

    pub fn cancel_recv(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.cancel_recv(event_loop));
    }

    pub fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) {
        self.apply(|s| s.send(event_loop, msg));
    }

    pub fn send_nb(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) {
        self.apply(|s| s.send_nb(event_loop, msg));
    }

    pub fn cancel_send(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.cancel_send(event_loop));
    }

    pub fn resync_readiness(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.resync_readiness(event_loop));
    }

    pub fn close(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.close(event_loop));
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

    fn subscribe(&self, event_loop: &mut EventLoop, events: mio::EventSet, opt: mio::PollOpt) -> io::Result<()> {
        let io = self.connection.as_evented();
        let events = events | mio::EventSet::hup() | mio::EventSet::error();

        event_loop.register(io, self.token, events, opt)
    }

    fn resubscribe(&self, event_loop: &mut EventLoop, events: mio::EventSet, opt: mio::PollOpt) -> io::Result<()> {
        let io = self.connection.as_evented();
        let events = events | mio::EventSet::hup() | mio::EventSet::error();

        event_loop.reregister(io, self.token, events, opt)
    }

    fn unsubscribe(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        event_loop.deregister(self.connection.as_evented())
    }

    fn send_sig(&self, sig: PipeEvtSignal) -> io::Result<()> {
        let evt_sig = EvtSignal::Pipe(self.token, sig);
        let loop_sig = EventLoopSignal::Evt(evt_sig);
        let send_res = self.sig_sender.send(loop_sig);

        send_res.map_err(|e| global::convert_notify_err(e))
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// STATE DEFINITION                                                          //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
trait PipeState {
    fn name(&self) -> &'static str;

    fn open(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn on_open_ack(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        box Dead
    }

    fn recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn cancel_recv(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn send(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        box Dead
    }

    fn send_nb(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        box Dead
    }

    fn cancel_send(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn resync_readiness(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn close(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn on_error(self: Box<Self>, _: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        debug!("State '{}' failed: {:?}", self.name(), err);
        box Dead
    }
}

trait LivePipeState {
    fn body<'a>(&'a self) -> &'a PipeBody;

    fn debug(&self, log: &str) {
        debug!("[{:?}] {}", self.body().token().as_usize(), log)
    }

    fn send_sig(&self, sig: PipeEvtSignal) -> io::Result<()> {
        self.body().send_sig(sig)
    }

    fn resync_readiness(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body().resubscribe(
            event_loop, 
            mio::EventSet::readable() | mio::EventSet::writable(), 
            mio::PollOpt::edge())
    }

    fn close(&self, event_loop: &mut EventLoop) -> Box<PipeState> {
        let _ = self.body().unsubscribe(event_loop);
        box Dead
    }
}

fn transition<F, T>(f: Box<F>) -> Box<T> where
    F : PipeState,
    T : From<F>,
    T : PipeState
{
    box From::from(*f)
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
// HANDSHAKE                                                                 //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////
fn create_handshake(protocol_id: u16) -> [u8; 8] {
    // handshake is Zero, 'S', 'P', Version, Proto[2], Rsvd[2]
    let mut handshake = [0, 83, 80, 0, 0, 0, 0, 0];
    BigEndian::write_u16(&mut handshake[4..6], protocol_id);
    handshake
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

    fn open(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.body.subscribe(event_loop, mio::EventSet::writable(), mio::PollOpt::level());

        transition_if_ok::<Initial, HandshakeTx>(self, res, event_loop)
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
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
        let handshake = create_handshake(self.protocol_id);
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
        self.body.resubscribe(event_loop, mio::EventSet::readable(), mio::PollOpt::level())
    }

    fn subscribe_to_writable(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.resubscribe(event_loop, mio::EventSet::writable(), mio::PollOpt::level())
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

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
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
        let expected_handshake = create_handshake(self.protocol_peer_id);

        if handshake == &expected_handshake {
            Ok(())
        } else {
            error!("expected '{:?}' but received '{:?}' !", expected_handshake, handshake);
            Err(io::Error::new(io::ErrorKind::InvalidData, "received bad handshake"))
        }
    }

    fn subscribe_to_readable(&self, event_loop: &mut EventLoop) -> io::Result<()> {
        self.body.resubscribe(event_loop, mio::EventSet::readable(), mio::PollOpt::level())
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

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
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

    fn on_open_ack(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.as_ref().resync_readiness(event_loop);

        transition_if_ok::<Activable, Idle>(self, res, event_loop)
    }

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        self
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
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
        box Receiving::from(*self, op)
    }

    fn sent_msg(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.send_sig(PipeEvtSignal::MsgSnd);

        no_transition_if_ok(self, res, event_loop)
    }

    fn sending_msg(self: Box<Self>, _: &mut EventLoop, op: send::SendOperation) -> Box<PipeState> {
        box Sending::from(*self, op)
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
        let mut operation = send::SendOperation::new(msg);

        match operation.send(self.body.connection()) {
            Ok(true)  => self.sent_msg(event_loop),
            Ok(false) => self.sending_msg(event_loop, operation),
            Err(e)    => self.on_error(event_loop, e)
        }
    }

    fn send_nb(mut self: Box<Self>, event_loop: &mut EventLoop, msg: Rc<Message>) -> Box<PipeState> {
        match send::send_nb(self.body.connection(), msg) {
            Ok(true)  => self,
            Ok(false) => self.on_error(event_loop, global::would_block_io_error("Non blocking send requested, but would block.")),
            Err(e)    => self.on_error(event_loop, e)
        }
    }

    fn cancel_send(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        self
    }

    fn resync_readiness(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.as_ref().resync_readiness(event_loop);

        no_transition_if_ok(self, res, event_loop)
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
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
            return box Idle { body: self.body };
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

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
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
            return box Idle { body: self.body };
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

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
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

#[cfg(test)]
mod tests {

    #[test]
    fn can_compare_array_slices() {
        let handshake1 : [u8; 8]= [0, 1, 2, 3, 4, 5, 6, 7];
        let handshake2 : [u8; 8]= [0, 1, 2, 3, 4, 5, 6, 7];
        let handshake3 : [u8; 8]= [0, 0, 0, 3, 4, 5, 6, 7];

        assert!(&handshake1 == &handshake2);
        assert!(&handshake1 != &handshake3);
        assert!(&handshake2 != &handshake3);
    }
}