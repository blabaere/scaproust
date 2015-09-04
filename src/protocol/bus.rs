// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;
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

pub struct Bus {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<Sender<SocketEvt>>,
    cancel_send_timeout: Option<EventLoopAction>,
    cancel_recv_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>,
    pending_recv: bool
}

impl Bus {
    pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Bus {
        Bus { 
            pipes: HashMap::new(),
            evt_sender: evt_tx,
            cancel_send_timeout: None,
            cancel_recv_timeout: None,
            pending_send: None,
            pending_recv: false
        }
    }

    fn on_msg_send_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_send_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_send = None;
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);
        for (_, pipe) in self.pipes.iter_mut() {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));
        for (_, pipe) in self.pipes.iter_mut() {
            pipe.cancel_send();
        }
    }

    fn process_send_result(&mut self, event_loop: &mut EventLoop, sent: bool, _: Option<mio::Token>) {
        if sent {
            let total_count = self.pipes.len();
            let done_count = self.pipes.values().filter(|p| p.is_send_finished()).count();

            if total_count == done_count {
                self.on_msg_send_finished_ok(event_loop);
            }
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

        for (_, pipe) in self.pipes.iter_mut() {
            pipe.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgNotRecv(err));

        for (_, pipe) in self.pipes.iter_mut() {
            pipe.cancel_recv();
        }
    }

    fn on_msg_recv_started(&mut self, token: mio::Token) {
        self.pending_recv = false;
        for (_, pipe) in self.pipes.iter_mut() {
            if pipe.token() == token {
                continue;
            } else {
                pipe.discard_recv();
            }
        }
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

impl Protocol for Bus {
    fn id(&self) -> u16 {
        SocketType::Bus.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Bus.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        self.cancel_send_timeout = Some(cancel_timeout);

        let mut sent = false;
        let mut sending = None;
        let msg = Rc::new(msg);

        for (_, pipe) in self.pipes.iter_mut() {
            match pipe.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = Some(pipe.token()),
                _ => continue
            }
        }

        self.pending_send = Some(msg);
        self.process_send_result(event_loop, sent, sending);
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err);
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.cancel_recv_timeout = Some(cancel_timeout);

        let mut received = None;
        let mut receiving = None;
        for (_, pipe) in self.pipes.iter_mut() {
            match pipe.recv() {
                Ok(RecvStatus::Completed(msg)) => received = Some(msg),
                Ok(RecvStatus::InProgress)     => receiving = Some(pipe.token()),
                _ => continue
            }
            break;
        }

        self.pending_recv = true;
        self.process_recv_result(event_loop, received, receiving);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_finished_err(event_loop, err);
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut sent = false;
        let mut sending = None;
        let mut received = None;
        let mut receiving = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            let has_pending_send = self.pending_send.is_some();
            let has_pending_recv = self.pending_recv;

            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r;

            if has_pending_send && !sent && pipe.can_resume_send() {
                let msg = self.pending_send.as_ref().unwrap();

                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(pipe.token()),
                    _ => {}
                }
            }

            if has_pending_recv && received.is_none() && pipe.can_resume_recv() {
                match try!(pipe.recv()) {
                    RecvStatus::Completed(msg) => received = Some(msg),
                    RecvStatus::InProgress     => receiving = Some(pipe.token()),
                    _ => {}
                }
            }
        }

        self.process_send_result(event_loop, sent, sending);
        self.process_recv_result(event_loop, received, receiving);

        Ok(())

    }
}
