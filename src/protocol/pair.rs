// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;
use pipe2::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

pub struct Pair {
    pipe: Option<Pipe>,
    evt_sender: Rc<Sender<SocketEvt>>,
    cancel_send_timeout: Option<EventLoopAction>,
    cancel_recv_timeout: Option<EventLoopAction>,
    pending_msg: Option<Rc<Message>>,
    pending_recv: bool
}

impl Pair {
    pub fn new(evt_sender: Rc<Sender<SocketEvt>>) -> Pair {
        Pair { 
            pipe: None,
            evt_sender: evt_sender,
            cancel_send_timeout: None,
            cancel_recv_timeout: None,
            pending_msg: None,
            pending_recv: false
        }
    }

    fn on_msg_send_finished(&mut self, 
        event_loop: &mut EventLoop, 
        evt: SocketEvt, 
        timeout: Option<EventLoopAction>) {
        let _ = self.evt_sender.send(evt);

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_msg = None;
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop) {
        let evt = SocketEvt::MsgSent;
        let timeout = self.cancel_send_timeout.take();

        self.on_msg_send_finished(event_loop, evt, timeout);

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        let evt = SocketEvt::MsgNotSent(err);
        let timeout = self.cancel_send_timeout.take();

        self.on_msg_send_finished(event_loop, evt, timeout);

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.cancel_send(); 
        }
    }

    fn on_msg_send_started(&mut self) {
        self.pending_msg = None;
    }

    fn process_send_result(&mut self, event_loop: &mut EventLoop, sent: bool, sending: bool) {
        if sent {
            self.on_msg_send_finished_ok(event_loop);
        } else if sending {
            self.on_msg_send_started();
        }
    }

    fn on_msg_recv_finished(&mut self, 
        event_loop: &mut EventLoop, 
        evt: SocketEvt, 
        timeout: Option<EventLoopAction>) {
        let _ = self.evt_sender.send(evt);

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_recv = false;
    }

    fn on_msg_recv_finished_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
        let evt = SocketEvt::MsgRecv(msg);
        let timeout = self.cancel_recv_timeout.take();

        self.on_msg_recv_finished(event_loop, evt, timeout);

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        let evt = SocketEvt::MsgNotRecv(err);
        let timeout = self.cancel_recv_timeout.take();

        self.on_msg_recv_finished(event_loop, evt, timeout);

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.cancel_recv(); 
        }
    }

    fn on_msg_recv_started(&mut self, _: mio::Token) {
        self.pending_recv = false;
    }

    fn process_recv_result(&mut self, event_loop: &mut EventLoop, received: Option<Message>, receiving: Option<mio::Token>) {
        if let Some(msg) = received {
            self.on_msg_recv_finished_ok(event_loop, msg);
        }
        else if let Some(token) = receiving {
            self.on_msg_recv_started(token);
        }
    }
}

impl Protocol for Pair {

    fn id(&self) -> u16 {
        SocketType::Pair.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Pair.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipe = Some(Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        if Some(token) == self.pipe.as_ref().map(|p| p.token()) {
            self.pipe.take().map(|p| p.remove())
        } else {
            None
        }
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
        self.cancel_send_timeout = Some(cancel_timeout);

        if self.pipe.is_none() {
            self.pending_msg = Some(Rc::new(msg));
            return;
        }

        let mut pipe = self.pipe.take().unwrap();
        let mut sent = false;
        let mut sending = false;
        let msg = Rc::new(msg);

        match pipe.send(msg.clone()) {
            Ok(SendStatus::Completed)  => sent = true,
            Ok(SendStatus::InProgress) => sending = true,
            _ => {}
        }

        self.pipe = Some(pipe);
        self.pending_msg = Some(msg);
        self.process_send_result(event_loop, sent, sending);
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err);
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.cancel_recv_timeout = Some(cancel_timeout);

        if self.pipe.is_none() {
            self.pending_recv = true;
            return;
        }

        let mut pipe = self.pipe.take().unwrap();
        let mut received = None;
        let mut receiving = None;

        match pipe.recv() {
            Ok(RecvStatus::Completed(msg)) => received = Some(msg),
            Ok(RecvStatus::InProgress)     => receiving = Some(pipe.token()),
            _ => {}
        }

        self.pipe = Some(pipe);
        self.pending_recv = true;
        self.process_recv_result(event_loop, received, receiving);
    }
    
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_finished_err(event_loop, err);
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        if self.pipe.is_none() {
            return Ok(());
        }
        if Some(token) != self.pipe.as_ref().map(|p| p.token()) {
            return Ok(());
        }

        let mut pipe = self.pipe.take().unwrap();
        let (sent, received) = try!(pipe.ready(event_loop, events));

        let has_pending_send = self.pending_msg.is_some();
        let mut sent = sent;
        let mut sending = false;

        if has_pending_send && !sent && pipe.can_resume_send() {
            let msg = self.pending_msg.as_ref().unwrap();

            match try!(pipe.send(msg.clone())) {
                SendStatus::Completed  => sent = true,
                SendStatus::InProgress => sending = true,
                _ => {}
            }
        }

        self.process_send_result(event_loop, sent, sending);

        let has_pending_recv = self.pending_recv;
        let mut received = received;
        let mut receiving = None;

        if has_pending_recv && received.is_none() && pipe.can_resume_recv() {
            match try!(pipe.recv()) {
                RecvStatus::Completed(msg) => received = Some(msg),
                RecvStatus::InProgress     => receiving = Some(token),
                _ => {}
            }
        }

        self.process_recv_result(event_loop, received, receiving);

        self.pipe = Some(pipe);

        Ok(())
    }
}