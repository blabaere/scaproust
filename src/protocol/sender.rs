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

type PipeHashMap = HashMap<mio::Token, Pipe>;

pub fn new_unicast_msg_sender(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> PolyadicMsgSender<UnicastSendingStrategy> {
    PolyadicMsgSender::new(evt_tx, UnicastSendingStrategy)
}

pub fn new_multicast_msg_sender(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> PolyadicMsgSender<MulticastSendingStrategy> {
    PolyadicMsgSender::new(evt_tx, MulticastSendingStrategy)
}

pub fn new_return_sender(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> ReturnSender {
    ReturnSender::new(evt_tx)
}

pub trait MsgSendingStrategy {
    // returns: finished, keep msg pending
    // finished: msg was sent, raise success notification
    // keep: msg should be kept for other pipes

    // send the message according to the strategy specification
    fn send(&self, msg: Rc<Message>, pipes: &mut HashMap<mio::Token, Pipe>) -> (bool, bool);

    // the specified pipe has successfully sent the current message
    fn sent_by(&self, token: mio::Token, pipes: &mut PipeHashMap) -> (bool, bool);

    // resume sending on the specified pipe
    fn resume_send(&self, msg: &Rc<Message>, token: mio::Token, pipes: &mut PipeHashMap) -> io::Result<(bool, bool)>;
}

pub struct PolyadicMsgSender<TStrategy : MsgSendingStrategy> {
    sending_strategy: TStrategy,
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>
}

impl<TStrategy : MsgSendingStrategy> PolyadicMsgSender<TStrategy> {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>, sending_strategy: TStrategy) -> PolyadicMsgSender<TStrategy> {
        PolyadicMsgSender {
            sending_strategy: sending_strategy,
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_send: None
        }
    }

    pub fn send(&mut self,
        event_loop: &mut EventLoop,
        msg: Message,
        cancel_timeout: EventLoopAction,
        pipes: &mut PipeHashMap) {

        self.cancel_timeout = Some(cancel_timeout);

        let msg = Rc::new(msg);
        let (finished, keep) = self.sending_strategy.send(msg.clone(), pipes);

        if finished {
            self.on_msg_send_finished_ok(event_loop, pipes);
        } else if keep {
            self.pending_send = Some(msg);
        }
    }

    pub fn sent_by(&mut self,
        event_loop: &mut EventLoop, 
        token: mio::Token,
        pipes: &mut PipeHashMap) {

        let (finished, keep) = self.sending_strategy.sent_by(token, pipes);

        if finished {
            self.on_msg_send_finished_ok(event_loop, pipes);
        } else if !keep {
            self.pending_send = None;
        }
    }

    pub fn resume_send(&mut self,
        event_loop: &mut EventLoop,
        token: mio::Token,
        pipes: &mut PipeHashMap) -> io::Result<()> {

        if self.pending_send.is_none() {
            return Ok(());
        }

        let msg = self.pending_send.take().unwrap();
        let (finished, keep) = try!(self.sending_strategy.resume_send(&msg, token, pipes));

        if finished {
            self.on_msg_send_finished_ok(event_loop, pipes);
        } else if keep {
            self.pending_send = Some(msg);
        }

        Ok(())
    }

    pub fn on_send_timeout(&mut self, event_loop: &mut EventLoop, pipes: &mut HashMap<mio::Token, Pipe>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err, pipes);
    }

    pub fn on_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipes: &mut PipeHashMap) {
        self.on_msg_send_finished_err(event_loop, err, pipes);
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop, pipes: &mut PipeHashMap) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);
        for (_, pipe) in pipes.iter_mut() {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipes: &mut PipeHashMap) {
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

pub struct UnicastSendingStrategy;

impl UnicastSendingStrategy {

    fn on_msg_send_started(&self, token: mio::Token, pipes: &mut PipeHashMap) {
        let filter_other = |p: &&mut Pipe| p.token() != token;
        let mut other_pipes = pipes.iter_mut().map(|(_, p)| p).filter(filter_other);

        while let Some(pipe) = other_pipes.next() {
            pipe.discard_send(); 
        }
    }

    fn is_finished_or_keep(&self, sent: bool, sending: Option<mio::Token>, pipes: &mut PipeHashMap) -> (bool, bool) {
        if let Some(token) = sending {
            self.on_msg_send_started(token, pipes);

            (false, false)
        } else {
            (sent, !sent)
        }
    }
}

impl MsgSendingStrategy for UnicastSendingStrategy {
    fn send(&self, msg: Rc<Message>, pipes: &mut PipeHashMap) -> (bool, bool) {
        let mut sent = false;
        let mut sending = None;

        for (_, pipe) in pipes.into_iter() {
            match pipe.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = Some(pipe.token()),
                _ => continue
            }
            break;
        }

        self.is_finished_or_keep(sent, sending, pipes)
    }

    fn sent_by(&self, _: mio::Token, _: &mut PipeHashMap) -> (bool, bool) {
        (true, false)
    }

    fn resume_send(&self, msg: &Rc<Message>, token: mio::Token, pipes: &mut PipeHashMap) -> io::Result<(bool, bool)> {
        let mut sent = false;
        let mut sending = None;

        if let Some(pipe) = pipes.get_mut(&token) {
            if pipe.can_resume_send() {
                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(token),
                    _ => {}
                }
            }
        }

        Ok(self.is_finished_or_keep(sent, sending, pipes))
    }
}

pub struct MulticastSendingStrategy;

impl MulticastSendingStrategy {
    fn is_finished_or_keep(&self, sent: bool, pipes: &mut PipeHashMap) -> (bool, bool) {
        if sent {
            let total_count = pipes.len();
            let done_count = pipes.values().filter(|p| p.is_send_finished()).count();
            let finished = total_count == done_count;

            (finished, !finished)
        } else {
            (false, true)
        }
    }
}

impl MsgSendingStrategy for MulticastSendingStrategy {
    fn send(&self, msg: Rc<Message>, pipes: &mut PipeHashMap) -> (bool, bool) {
        let mut sent = false;

        for (_, pipe) in pipes.into_iter() {
            match pipe.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                _ => continue
            }
        }

        self.is_finished_or_keep(sent, pipes)
    }

    fn sent_by(&self, _: mio::Token, pipes: &mut PipeHashMap) -> (bool, bool) {
        self.is_finished_or_keep(true, pipes)
     }

    fn resume_send(&self, msg: &Rc<Message>, token: mio::Token, pipes: &mut PipeHashMap) -> io::Result<(bool, bool)> {
        let mut sent = false;

        if let Some(pipe) = pipes.get_mut(&token) {
            if pipe.can_resume_send() {
                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    _ => {}
                }
            }
        }

        Ok(self.is_finished_or_keep(sent, pipes))
     }
}

// I gave a letter to the postman,
// He put it in his sack.
// And by the early next morning,
// He brought my letter back.
pub struct ReturnSender {
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>
}

impl ReturnSender {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> ReturnSender {
        ReturnSender {
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_send: None
        }
    }

    pub fn send(&mut self,
        event_loop: &mut EventLoop,
        msg: Message,
        cancel_timeout: EventLoopAction,
        token: mio::Token,
        pipes: &mut PipeHashMap) {

        self.cancel_timeout = Some(cancel_timeout);

        let msg = Rc::new(msg);
        let (finished, keep) = self.send_to(msg.clone(), token, pipes);

        if finished {
            self.on_msg_send_finished_ok(event_loop, pipes);
        } else if keep {
            self.pending_send = Some(msg);
        }
    }

    fn send_to(&self, msg: Rc<Message>, token: mio::Token, pipes: &mut PipeHashMap) -> (bool, bool) {
        let mut sent = false;
        let mut sending = None;

        if let Some(pipe) = pipes.get_mut(&token) {
            match pipe.send(msg) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = Some(pipe.token()),
                _ => {}
            }
        }

        self.is_finished_or_keep(sent, sending, pipes)
    }

    fn is_finished_or_keep(&self, sent: bool, sending: Option<mio::Token>, pipes: &mut PipeHashMap) -> (bool, bool) {
        if let Some(token) = sending {
            self.on_msg_send_started(token, pipes);

            (false, false)
        } else {
            (sent, !sent)
        }
    }

    fn on_msg_send_started(&self, token: mio::Token, pipes: &mut PipeHashMap) {
        let filter_other = |p: &&mut Pipe| p.token() != token;
        let mut other_pipes = pipes.iter_mut().map(|(_, p)| p).filter(filter_other);

        while let Some(pipe) = other_pipes.next() {
            pipe.discard_send(); 
        }
    }

    pub fn sent_by(&mut self,
        event_loop: &mut EventLoop, 
        _: mio::Token,
        pipes: &mut PipeHashMap) {

        self.on_msg_send_finished_ok(event_loop, pipes);
    }

    pub fn resume_send(&mut self,
        event_loop: &mut EventLoop,
        token: mio::Token,
        pipes: &mut PipeHashMap) -> io::Result<()> {

        if self.pending_send.is_none() {
            return Ok(());
        }

        let msg = self.pending_send.take().unwrap();
        let (finished, keep) = try!(self.resume_send_to(&msg, token, pipes));

        if finished {
            self.on_msg_send_finished_ok(event_loop, pipes);
        } else if keep {
            self.pending_send = Some(msg);
        }

        Ok(())
    }

    fn resume_send_to(&self, msg: &Rc<Message>, token: mio::Token, pipes: &mut PipeHashMap) -> io::Result<(bool, bool)> {
        let mut sent = false;
        let mut sending = None;

        if let Some(pipe) = pipes.get_mut(&token) {
            if pipe.can_resume_send() {
                match try!(pipe.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(token),
                    _ => {}
                }
            }
        }

        Ok(self.is_finished_or_keep(sent, sending, pipes))
    }

    pub fn on_send_timeout(&mut self, event_loop: &mut EventLoop, pipes: &mut HashMap<mio::Token, Pipe>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err, pipes);
    }

    pub fn on_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipes: &mut PipeHashMap) {
        self.on_msg_send_finished_err(event_loop, err, pipes);
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop, pipes: &mut PipeHashMap) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);
        for (_, pipe) in pipes.iter_mut() {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipes: &mut PipeHashMap) {
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

pub struct SendToSingle {
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>
}

impl SendToSingle {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> SendToSingle {
        SendToSingle {
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_send: None
        }
    }

    pub fn send(&mut self,
        event_loop: &mut EventLoop,
        msg: Message,
        cancel_timeout: EventLoopAction,
        pipe_cell: Option<&mut Pipe>) {

        self.cancel_timeout = Some(cancel_timeout);

        if pipe_cell.is_none() {
            return self.pending_send = Some(Rc::new(msg));
        }

        let mut pipe = pipe_cell.unwrap();
        let mut sent = false;
        let mut sending = false;
        let msg = Rc::new(msg);

        match pipe.send(msg.clone()) {
            Ok(SendStatus::Completed)  => sent = true,
            Ok(SendStatus::InProgress) => sending = true,
            _ => {}
        }

        self.pending_send = Some(msg);
        self.process_send_result(event_loop, sent, sending, Some(pipe));
    }

    pub fn on_send_timeout(&mut self, event_loop: &mut EventLoop, pipe_cell: Option<&mut Pipe>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err, pipe_cell);
    }

    pub fn on_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipe_cell: Option<&mut Pipe>) {
        self.on_msg_send_finished_err(event_loop, err, pipe_cell);
    }

    pub fn on_pipe_ready(&mut self, event_loop: &mut EventLoop, _: mio::Token, sent: bool, pipe: &mut Pipe) -> io::Result<()> {
        let has_pending_send = self.pending_send.is_some();
        let mut sent = sent;
        let mut sending = false;

        if has_pending_send && !sent && pipe.can_resume_send() {
            let msg = self.pending_send.as_ref().unwrap();

            match try!(pipe.send(msg.clone())) {
                SendStatus::Completed  => sent = true,
                SendStatus::InProgress => sending = true,
                _ => {}
            }
        }

        self.process_send_result(event_loop, sent, sending, Some(pipe));

        Ok(())
    }

    fn process_send_result(&mut self, event_loop: &mut EventLoop, sent: bool, sending: bool, pipe_cell: Option<&mut Pipe>) {
        if sent {
            self.on_msg_send_finished_ok(event_loop, pipe_cell);
        } else if sending {
            self.on_msg_send_started();
        }
    }

    fn on_msg_send_started(&mut self) {
        self.pending_send = None;
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop, pipe_cell: Option<&mut Pipe>) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);

        if let Some(pipe) = pipe_cell {
            pipe.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipe_cell: Option<&mut Pipe>) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));

        if let Some(pipe) = pipe_cell {
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
