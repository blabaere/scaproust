// Copyright 015 Copyright (c) 015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };

use mio;

use EventLoop;
use Message;
use transport::Connection;
use global;

// A pipe is responsible for handshaking with its peer and transfering raw messages over a connection.
// That means send/receive size prefix and then message payload
// according to the connection readiness and the requested operation progress if any
pub struct Pipe {
    addr: Option<String>,
    state: Option<Box<PipeState>>
}
/*
pub enum SendStatus {
    Postponed(Rc<Message>), // Message can't be sent at the moment : Handshake in progress or would block
    Completed,              // Message has been successfully sent
    InProgress              // Message has been partially sent, will finish later
}

pub enum RecvStatus {
    Postponed,              // Message can't be read at the moment : Handshake in progress or would block
    Completed(Message),     // Message has been successfully read
    InProgress              // Message has been partially read, will finish later
}
*/
impl Pipe {

    pub fn new(
        token: mio::Token,
        addr: Option<String>,
        ids: (u16, u16),
        conn: Box<Connection>) -> Pipe {

        let (protocol_id, protocol_peer_id) = ids;
        let state = Initial::new(token, protocol_id, protocol_peer_id, conn);

        Pipe {
            addr: addr,
            state: Some(Box::new(state))
        }
    }

    fn on_state_transition<F>(&mut self, transition: &mut F) where F : FnMut(Box<PipeState>) -> Box<PipeState> {
        if let Some(state) = self.state.take() {
            self.state = Some(transition(state));
        }
    }

    pub fn open(&mut self, event_loop: &mut EventLoop) {
        self.on_state_transition(&mut |s: Box<PipeState>| s.open(event_loop));
    }

    pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        self.on_state_transition(&mut |s: Box<PipeState>| s.ready(event_loop, events));
    }

    pub fn addr(self) -> Option<String> {
        self.addr
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
    token: mio::Token,
    protocol_id: u16,
    protocol_peer_id: u16,
    connection: Box<Connection>,
}

impl Initial {
    fn new(
        tok: mio::Token, 
        p_id: u16,
        peer_p_id: u16,
        conn: Box<Connection>) -> Initial {
        Initial { 
            token: tok,
            protocol_id: p_id,
            protocol_peer_id: peer_p_id,
            connection: conn
        }
    }

    fn register_for_write(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        let interest = mio::EventSet::error() | mio::EventSet::hup() | mio::EventSet::writable();
        let poll = mio::PollOpt::edge() | mio::PollOpt::oneshot();

        event_loop.register(
            self.connection.as_evented(), 
            self.token, 
            interest, 
            poll)

    }
}

impl PipeState for Initial {
    fn open(mut self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let registered = self.register_for_write(event_loop);

        transition_if_ok::<Initial, HandshakeTx>(self, registered, event_loop)
    }
}

struct HandshakeTx {
    token: mio::Token,
    protocol_id: u16,
    protocol_peer_id: u16,
    connection: Box<Connection>,
}

impl From<Initial> for HandshakeTx {
    fn from(state: Initial) -> HandshakeTx {
        HandshakeTx {
            token: state.token,
            protocol_id: state.protocol_id,
            protocol_peer_id: state.protocol_peer_id,
            connection: state.connection
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
            self.connection.try_write(&handshake).
            and_then(|w| self.check_sent_handshake(w)));
        debug!("[{:?}] handshake sent.", self.token);
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
        register_for_write(event_loop, &*self.connection, self.token)
    }

    fn register_for_read(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        register_for_read(event_loop, &*self.connection, self.token)
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
}

struct HandshakeRx {
    token: mio::Token,
    protocol_id: u16,
    protocol_peer_id: u16,
    connection: Box<Connection>,
}

impl From<HandshakeTx> for HandshakeRx {
    fn from(state: HandshakeTx) -> HandshakeRx {
        HandshakeRx {
            token: state.token,
            protocol_id: state.protocol_id,
            protocol_peer_id: state.protocol_peer_id,
            connection: state.connection
        }
    }
}

impl HandshakeRx {

    fn register_for_none(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        register_for_event(event_loop, &*self.connection, self.token, mio::EventSet::none())
    }

    fn register_for_read(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
        register_for_read(event_loop, &*self.connection, self.token)
    }

    fn read_handshake(&mut self) -> io::Result<()> {
        let mut handshake = [0u8; 8];
        try!(
            self.connection.try_read(&mut handshake).
            and_then(|_| self.check_received_handshake(&handshake)));
        debug!("[{:?}] handshake received.", self.token);
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
}

struct Idle {
    token: mio::Token,
    protocol_id: u16,
    protocol_peer_id: u16,
    connection: Box<Connection>,
}

impl From<HandshakeRx> for Idle {
    fn from(state: HandshakeRx) -> Idle {
        Idle {
            token: state.token,
            protocol_id: state.protocol_id,
            protocol_peer_id: state.protocol_peer_id,
            connection: state.connection
        }
    }
}

impl PipeState for Idle {
}

struct Dead;

impl PipeState for Dead {
}

fn register_for_write(event_loop: &mut EventLoop, conn: &Connection, tok: mio::Token) -> io::Result<()> {
    register_for_event(event_loop, conn, tok, mio::EventSet::writable())
}

fn register_for_read(event_loop: &mut EventLoop, conn: &Connection, tok: mio::Token) -> io::Result<()> {
    register_for_event(event_loop, conn, tok, mio::EventSet::readable())
}

fn register_for_event(
    event_loop: &mut EventLoop,
    conn: &Connection,
    tok: mio::Token,
    event: mio::EventSet) -> io::Result<()> {

    let interest = mio::EventSet::error() | mio::EventSet::hup() | event;
    let poll = mio::PollOpt::edge() | mio::PollOpt::oneshot();

    event_loop.reregister(conn.as_evented(), tok, interest, poll)
}
