// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::iter::Iterator;
use std::io;

use mio;

use super::Protocol;
use pipe::*;
use endpoint::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

pub struct SendToAny {
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>
}

impl SendToAny {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> SendToAny {
        SendToAny {
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_send: None
        }
    }

    pub fn send(&mut self,
        event_loop: &mut EventLoop,
        msg: Message,
        cancel_timeout: EventLoopAction,
        pipes: &mut HashMap<mio::Token, Pipe>) {

        self.cancel_timeout = Some(cancel_timeout);

        let mut sent = false;
        let mut sending = None;
        let msg = Rc::new(msg);

        for (_, pipe) in pipes.into_iter() {
            match pipe.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = Some(pipe.token()),
                _ => continue
            }
            break;
        }

        self.pending_send = Some(msg);
        self.process_send_result(event_loop, sent, sending, pipes);
    }

    pub fn on_send_timeout(&mut self, event_loop: &mut EventLoop, pipes: &mut HashMap<mio::Token, Pipe>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err, pipes);
    }

    pub fn on_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipes: &mut HashMap<mio::Token, Pipe>) {
        self.on_msg_send_finished_err(event_loop, err, pipes);
    }

    pub fn on_pipe_ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, sent: bool, pipes: &mut HashMap<mio::Token, Pipe>) -> io::Result<()> {
        let has_pending_send = self.pending_send.is_some();
        let mut sent = sent;
        let mut sending = None;

        if let Some(pipe) = pipes.get_mut(&token) {
            if has_pending_send && !sent && pipe.can_resume_send() {
                let msg = self.pending_send.as_ref().unwrap();

                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(token),
                    _ => {}
                }
            }
        }

        self.process_send_result(event_loop, sent, sending, pipes);

        Ok(())
    }

    fn process_send_result(&mut self, event_loop: &mut EventLoop, sent: bool, sending: Option<mio::Token>, pipes: &mut HashMap<mio::Token, Pipe>) {
        if sent {
            self.on_msg_send_finished_ok(event_loop, pipes);
        } else if let Some(token) = sending {
            self.on_msg_send_started(token, pipes);
        }
    }

    fn on_msg_send_started(&mut self, token: mio::Token, pipes: &mut HashMap<mio::Token, Pipe>) {
        self.pending_send = None;
        for (_, pipe) in pipes.iter_mut() {
            if pipe.token() == token {
                continue;
            } else {
                pipe.discard_send();
            }
        }
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop, pipes: &mut HashMap<mio::Token, Pipe>) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);
        for (_, pipe) in pipes.iter_mut() {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipes: &mut HashMap<mio::Token, Pipe>) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));
        for (_, pipe) in pipes.iter_mut() {
            pipe.cancel_send();
        }
    }

    fn on_msg_send_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
        
        self.pending_send = None;
    }
}