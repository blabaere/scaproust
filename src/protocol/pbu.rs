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
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

pub struct Pub {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<Sender<SocketEvt>>,
    cancel_send_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>
}

impl Pub {
    pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Pub {
        Pub { 
            pipes: HashMap::new(),
            evt_sender: evt_tx,
            cancel_send_timeout: None,
            pending_send: None
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
}

impl Protocol for Pub {
    fn id(&self) -> u16 {
        SocketType::Pub.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Sub.id()
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

    fn recv(&mut self, _: &mut EventLoop, _: EventLoopAction) {
        let err = other_io_error("recv not supported by protocol");
        let cmd = SocketEvt::MsgNotRecv(err);
        let _ = self.evt_sender.send(cmd);
    }
    
    fn on_recv_timeout(&mut self, _: &mut EventLoop) {
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let has_pending_send = self.pending_send.is_some();
        let mut sent = false;
        let mut sending = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            sent = try!(pipe.ready_tx(event_loop, events));

            if has_pending_send && !sent && pipe.can_resume_send() {
                let msg = self.pending_send.as_ref().unwrap();

                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(token),
                    _ => {}
                }
            }
        }

        self.process_send_result(event_loop, sent, sending);

        Ok(())
    }
}
