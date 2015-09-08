// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;

use mio;

use super::Protocol;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

use super::sender::SendToSingle;

pub struct Pair {
    pipe: Option<Pipe>,
    evt_sender: Rc<Sender<SocketEvt>>,
    cancel_recv_timeout: Option<EventLoopAction>,
    pending_recv: bool,
    msg_sender: SendToSingle
}

impl Pair {
    pub fn new(evt_sender: Rc<Sender<SocketEvt>>) -> Pair {
        Pair { 
            pipe: None,
            evt_sender: evt_sender.clone(),
            cancel_recv_timeout: None,
            pending_recv: false,
            msg_sender: SendToSingle::new(evt_sender)
        }
    }

    fn on_msg_recv_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_recv_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_recv = false;
    }

    fn on_msg_recv_finished_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgRecv(msg));

        if let Some(pipe) = self.pipe.as_mut() {
            pipe.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgNotRecv(err));

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

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        self.msg_sender.send(event_loop, msg, cancel_timeout, self.pipe.as_mut())
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_sender.on_send_timeout(event_loop, self.pipe.as_mut())
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
        let mut sent = false;
        let mut received = None;
        let mut receiving = None;

        if let Some(pipe) = self.pipe.as_mut() {
            let has_pending_recv = self.pending_recv;
            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r;

            if has_pending_recv && received.is_none() && pipe.can_resume_recv() {
                match try!(pipe.recv()) {
                    RecvStatus::Completed(msg) => received = Some(msg),
                    RecvStatus::InProgress     => receiving = Some(token),
                    _ => {}
                }
            }
        }

        if let Some(pipe) = self.pipe.as_mut() {
            try!(self.msg_sender.on_pipe_ready(event_loop, token, sent, pipe));
        }
        self.process_recv_result(event_loop, received, receiving);

        Ok(())
    }
}