// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;

use mio;

use super::Protocol;
use pipe2::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

pub struct Sub {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_recv: bool
}

impl Sub {
    pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Sub {
        Sub { 
            pipes: HashMap::new(),
            evt_sender: evt_sender,
            cancel_timeout: None,
            pending_recv: false
        }
    }

    fn on_msg_recv_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_timeout.take();

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

    // TODO filter message against subscriptions here
    // try again if message does not match
    fn process_recv_result(&mut self, event_loop: &mut EventLoop, received: Option<Message>, receiving: Option<mio::Token>) {
        if let Some(msg) = received {
            self.on_msg_recv_finished_ok(event_loop, msg);
        }
        else if let Some(token) = receiving {
            self.on_msg_recv_started(token);
        }
    }
}

impl Protocol for Sub {
    fn id(&self) -> u16 {
        SocketType::Sub.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Pub.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn send(&mut self, _: &mut EventLoop, _: Message, _: EventLoopAction) {
        let err = other_io_error("send not supported by protocol");
        let cmd = SocketEvt::MsgNotSent(err);
        let _ = self.evt_sender.send(cmd);
    }

    fn on_send_timeout(&mut self, _: &mut EventLoop) {
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.cancel_timeout = Some(cancel_timeout);

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
        let has_pending_recv = self.pending_recv;
        let mut received = None;
        let mut receiving = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            received = try!(pipe.ready_rx(event_loop, events));

            if has_pending_recv && received.is_none() && pipe.can_resume_recv() {
                match try!(pipe.recv()) {
                    RecvStatus::Completed(msg) => received = Some(msg),
                    RecvStatus::InProgress     => receiving = Some(token),
                    _ => {}
                }
            }
        }

        self.process_recv_result(event_loop, received, receiving);

        Ok(())
    }
}
