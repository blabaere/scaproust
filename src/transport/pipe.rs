// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use SocketType;
use EventLoop;
use Message;
use transport::{ Connection, Sender, Receiver };
use global;
use event_loop_msg::*;

/// A pipe is responsible for handshaking with its peer and transfering raw messages over a connection.
/// That means send/receive size prefix and then message payload.
/// This is done according to the connection readiness and the operation progress.
pub struct Pipe {
    token: mio::Token,
    addr: Option<String>,
    send_priority: u8,
    recv_priority: u8,
    state: Option<Box<PipeState>>
}

impl Pipe {

    pub fn new(
        tok: mio::Token,
        addr: Option<String>,
        socket_type: SocketType,
        priorities: (u8, u8),
        conn: Box<Connection>,
        sig_tx: mio::Sender<EventLoopSignal>,
        error_count: u32) -> Pipe {

        let (send_prio, recv_prio) = priorities;
        let state = Initial {
            body : PipeBody {
                token: tok,
                connection: conn,
                sig_sender: sig_tx
            },
            socket_type: socket_type,
            error_count: error_count
        };

        Pipe {
            token: tok,
            addr: addr,
            send_priority: send_prio,
            recv_priority: recv_prio,
            state: Some(box state)
        }
    }

    pub fn token(&self) -> mio::Token {
        self.token
    }

    pub fn can_recv(&self) -> bool {
        match self.state.as_ref().map(|s| s.can_recv()) {
            Some(true)  => true,
            Some(false) => false,
            None        => false
        }
    }

    pub fn can_send(&self) -> bool {
        match self.state.as_ref().map(|s| s.can_send()) {
            Some(true)  => true,
            Some(false) => false,
            None        => false
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

    pub fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) {
        self.apply(|s| s.send(event_loop, msg));
    }

    pub fn resync_readiness(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.resync_readiness(event_loop));
    }

    pub fn close(&mut self, event_loop: &mut EventLoop) {
        self.apply(|s| s.close(event_loop));
    }

    pub fn get_send_priority(&self) -> u8 {
        self.send_priority
    }

    pub fn get_recv_priority(&self) -> u8 {
        self.recv_priority
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

        send_res.map_err(global::convert_notify_err)
    }
}


/*****************************************************************************/
/*                                                                           */
/* STATE DEFINITION                                                          */
/*                                                                           */
/*****************************************************************************/
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

    fn send(self: Box<Self>, _: &mut EventLoop, _: Rc<Message>) -> Box<PipeState> {
        box Dead
    }

    fn resync_readiness(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn close(self: Box<Self>, _: &mut EventLoop) -> Box<PipeState> {
        box Dead
    }

    fn can_recv(&self) -> bool {
        false
    }

    fn can_send(&self) -> bool {
        false
    }

    fn on_error(self: Box<Self>, _: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        debug!("State '{}' failed: {:?}", self.name(), err);
        box Dead
    }
}

trait LivePipeState : PipeState {
    fn body(&self) -> &PipeBody;

    fn token(&self) -> mio::Token {
        self.body().token()
    }

    fn debug(&self, log: &str) {
        debug!("[{:?}] {}", self.token().as_usize(), log)
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
        let _ = self.send_sig(PipeEvtSignal::Closed);
        let _ = self.body().unsubscribe(event_loop);
        box Dead
    }

    fn on_error(&self, _: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        debug!("Live State '{}' failed: {:?}", self.name(), err);
        let count = self.get_error_count();
        let signal = PipeEvtSignal::Error(count);
        let _ = self.send_sig(signal);
        box Dead
    }

    fn get_error_count(&self) -> u32 {
        0
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

/*****************************************************************************/
/*                                                                           */
/* INITIAL STATE                                                             */
/*                                                                           */
/*****************************************************************************/
struct Initial {
    body: PipeBody, 
    socket_type: SocketType,
    error_count: u32
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

    fn on_error(self: Box<Self>, event_loop: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        self.as_ref().on_error(event_loop, err)
    }
}

impl LivePipeState for Initial {
    fn body(&self) -> &PipeBody {
        &self.body
    }

    fn get_error_count(&self) -> u32 {
        self.error_count + 1
    }
}

/*****************************************************************************/
/*                                                                           */
/* SEND HANDSHAKE STATE                                                      */
/*                                                                           */
/*****************************************************************************/
struct HandshakeTx {
    body: PipeBody, 
    socket_type: SocketType,
    error_count: u32
}

impl From<Initial> for HandshakeTx {
    fn from(state: Initial) -> HandshakeTx {
        HandshakeTx {
            body: state.body,
            socket_type: state.socket_type,
            error_count: state.error_count
        }
    }
}

impl HandshakeTx {

    fn write_handshake(&mut self) -> io::Result<()> {
        let connection = self.body.connection();
        let socket_type = self.socket_type;
        connection.send_handshake(socket_type)
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

    fn on_error(self: Box<Self>, event_loop: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        self.as_ref().on_error(event_loop, err)
    }
}

impl LivePipeState for HandshakeTx {
    fn body(&self) -> &PipeBody {
        &self.body
    }

    fn get_error_count(&self) -> u32 {
        self.error_count + 1
    }
}

/*****************************************************************************/
/*                                                                           */
/* RECV PEER HANDSHAKE STATE                                                 */
/*                                                                           */
/*****************************************************************************/
struct HandshakeRx {
    body: PipeBody, 
    socket_type: SocketType
}

impl From<HandshakeTx> for HandshakeRx {
    fn from(state: HandshakeTx) -> HandshakeRx {
        HandshakeRx {
            body: state.body,
            socket_type: state.socket_type
        }
    }
}

impl HandshakeRx {

    fn read_handshake(&mut self) -> io::Result<()> {
        let connection = self.body.connection();
        let socket_type = self.socket_type;
        connection.recv_handshake(socket_type)
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

    fn on_error(self: Box<Self>, event_loop: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        self.as_ref().on_error(event_loop, err)
    }
}

impl LivePipeState for HandshakeRx {
    fn body(&self) -> &PipeBody {
        &self.body
    }
}

/*****************************************************************************/
/*                                                                           */
/* HANDSHAKE ESTABLISHED STATE                                               */
/*                                                                           */
/*****************************************************************************/
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

        transition_if_ok::<Activable, Active>(self, res, event_loop)
    }

    fn ready(self: Box<Self>, _: &mut EventLoop, _: mio::EventSet) -> Box<PipeState> {
        self
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
    }

    fn on_error(self: Box<Self>, event_loop: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        self.as_ref().on_error(event_loop, err)
    }
}

impl LivePipeState for Activable {
    fn body(&self) -> &PipeBody {
        &self.body
    }
}

/*****************************************************************************/
/*                                                                           */
/* ACTIVE STATE                                                              */
/*                                                                           */
/*****************************************************************************/
struct Active {
    body: PipeBody,
    readable: bool,
    writable: bool
}

impl From<Activable> for Active {
    fn from(state: Activable) -> Active {
        Active {
            body: state.body,
            readable: false,
            writable: false
        }
    }
}

impl Active {
    fn on_recv_progress(&mut self, progress: io::Result<Option<Message>>) -> io::Result<()> {
        match progress {
            Ok(Some(msg)) => self.received_msg(msg),
            Ok(None) => self.receiving_msg(),
            Err(e) => Err(e)
        }
    }

    fn received_msg(&mut self, msg: Message) -> io::Result<()> {
        self.send_sig(PipeEvtSignal::RecvDone(msg))
    }

    fn receiving_msg(&mut self) -> io::Result<()> {
        self.readable = false;
        self.send_sig(PipeEvtSignal::RecvBlocked)
    }

    fn readable_changed(&mut self, readable: bool) -> io::Result<()> {
        if readable {
            if self.body.connection.has_pending_recv() {
                let progress = self.body.connection.resume_recv();
                return self.on_recv_progress(progress);
            } else {
                self.readable = true
            }
        } else {
            self.readable = false;
        }

        Ok(())
    }

    fn on_send_progress(&mut self, progress: io::Result<bool>) -> io::Result<()> {
        match progress {
            Ok(true) => self.sent_msg(),
            Ok(false) => self.sending_msg(),
            Err(e) => Err(e)
        }
    }

    fn sent_msg(&mut self) -> io::Result<()> {
        self.send_sig(PipeEvtSignal::SendDone)
    }

    fn sending_msg(&mut self) -> io::Result<()> {
        self.writable = false;
        self.send_sig(PipeEvtSignal::SendBlocked)
    }

    fn writable_changed(&mut self, writable: bool) -> io::Result<()> {
        if writable {
            if self.body.connection.has_pending_send() {
                let progress = self.body.connection.resume_send();
                return self.on_send_progress(progress);
            } else {
                self.writable = true
            }
        } else {
            self.writable = false;
        }

        Ok(())
    }
}

impl PipeState for Active {
    fn name(&self) -> &'static str {
        "Active"
    }

    fn ready(mut self: Box<Self>, event_loop: &mut EventLoop, events: mio::EventSet) -> Box<PipeState> {
        if events.is_hup() || events.is_error() {
            let _ = self.send_sig(PipeEvtSignal::Closed);
            self
        } else {
            let res = 
                self.readable_changed(events.is_readable()).and_then(|_|
                self.writable_changed(events.is_writable()));

            no_transition_if_ok(self, res, event_loop)
        }
    }

    fn recv(mut self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let progress = self.body.connection.start_recv();
        let res = self.on_recv_progress(progress);

        no_transition_if_ok(self, res, event_loop)
    }

    fn send(mut self: Box<Self>, event_loop: &mut EventLoop, msg: Rc<Message>) -> Box<PipeState> {
        let progress = self.body.connection.start_send(msg);
        let res = self.on_send_progress(progress);

        no_transition_if_ok(self, res, event_loop)
    }

    fn resync_readiness(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        let res = self.as_ref().resync_readiness(event_loop);

        no_transition_if_ok(self, res, event_loop)
    }

    fn can_recv(&self) -> bool {
        self.readable
    }

    fn can_send(&self) -> bool {
        self.writable
    }

    fn close(self: Box<Self>, event_loop: &mut EventLoop) -> Box<PipeState> {
        self.as_ref().close(event_loop)
    }

    fn on_error(self: Box<Self>, event_loop: &mut EventLoop, err: io::Error) -> Box<PipeState> {
        self.as_ref().on_error(event_loop, err)
    }
}

impl LivePipeState for Active {
    fn body(&self) -> &PipeBody {
        &self.body
    }
}

/*****************************************************************************/
/*                                                                           */
/* DEAD STATE                                                                */
/*                                                                           */
/*****************************************************************************/
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
        let handshake1: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
        let handshake2: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
        let handshake3: [u8; 8] = [0, 0, 0, 3, 4, 5, 6, 7];

        assert!(&handshake1 == &handshake2);
        assert!(&handshake1 != &handshake3);
        assert!(&handshake2 != &handshake3);
    }
}