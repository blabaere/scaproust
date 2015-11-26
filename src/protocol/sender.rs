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
use pipe::SendStatus;
use endpoint::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

type EndpointHashMap = HashMap<mio::Token, Endpoint>;

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
    // keep: msg should be kept for other endpoints

    // send the message according to the strategy specification
    fn send(&self, msg: Rc<Message>, endpoints: &mut HashMap<mio::Token, Endpoint>) -> (bool, bool);

    // the specified endpoint has successfully sent the current message
    fn sent_by(&self, token: mio::Token, endpoints: &mut EndpointHashMap) -> (bool, bool);

    // resume sending on the specified endpoint
    fn resume_send(&self, msg: &Rc<Message>, token: mio::Token, endpoints: &mut EndpointHashMap) -> io::Result<(bool, bool)>;
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
        endpoints: &mut EndpointHashMap) {

        self.cancel_timeout = Some(cancel_timeout);

        let msg = Rc::new(msg);
        let (finished, keep) = self.sending_strategy.send(msg.clone(), endpoints);

        if finished {
            self.on_msg_send_finished_ok(event_loop, endpoints);
        } else if keep {
            self.pending_send = Some(msg);
        }
    }

    pub fn sent_by(&mut self,
        event_loop: &mut EventLoop, 
        token: mio::Token,
        endpoints: &mut EndpointHashMap) {

        let (finished, keep) = self.sending_strategy.sent_by(token, endpoints);

        if finished {
            self.on_msg_send_finished_ok(event_loop, endpoints);
        } else if !keep {
            self.pending_send = None;
        }
    }

    pub fn resume_send(&mut self,
        event_loop: &mut EventLoop,
        token: mio::Token,
        endpoints: &mut EndpointHashMap) -> io::Result<()> {

        if self.pending_send.is_none() {
            return Ok(());
        }

        let msg = self.pending_send.take().unwrap();
        let (finished, keep) = try!(self.sending_strategy.resume_send(&msg, token, endpoints));

        if finished {
            self.on_msg_send_finished_ok(event_loop, endpoints);
        } else if keep {
            self.pending_send = Some(msg);
        }

        Ok(())
    }

    pub fn on_send_timeout(&mut self, event_loop: &mut EventLoop, endpoints: &mut HashMap<mio::Token, Endpoint>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err, endpoints);
    }

    pub fn on_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoints: &mut EndpointHashMap) {
        self.on_msg_send_finished_err(event_loop, err, endpoints);
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop, endpoints: &mut EndpointHashMap) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);
        for (_, endpoint) in endpoints.iter_mut() {
            endpoint.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoints: &mut EndpointHashMap) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));
        for (_, endpoint) in endpoints.iter_mut() {
            endpoint.cancel_send();
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

    fn on_msg_send_started(&self, token: mio::Token, endpoints: &mut EndpointHashMap) {
        let filter_other = |p: &&mut Endpoint| p.token() != token;
        let mut other_endpoints = endpoints.iter_mut().map(|(_, p)| p).filter(filter_other);

        while let Some(endpoint) = other_endpoints.next() {
            endpoint.discard_send(); 
        }
    }

    fn is_finished_or_keep(&self, sent: bool, sending: Option<mio::Token>, endpoints: &mut EndpointHashMap) -> (bool, bool) {
        if let Some(token) = sending {
            self.on_msg_send_started(token, endpoints);

            (false, false)
        } else {
            (sent, !sent)
        }
    }
}

impl MsgSendingStrategy for UnicastSendingStrategy {
    fn send(&self, msg: Rc<Message>, endpoints: &mut EndpointHashMap) -> (bool, bool) {
        let mut sent = false;
        let mut sending = None;

        for (_, endpoint) in endpoints.into_iter() {
            match endpoint.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = Some(endpoint.token()),
                _ => continue
            }
            break;
        }

        self.is_finished_or_keep(sent, sending, endpoints)
    }

    fn sent_by(&self, _: mio::Token, _: &mut EndpointHashMap) -> (bool, bool) {
        (true, false)
    }

    fn resume_send(&self, msg: &Rc<Message>, token: mio::Token, endpoints: &mut EndpointHashMap) -> io::Result<(bool, bool)> {
        let mut sent = false;
        let mut sending = None;

        if let Some(endpoint) = endpoints.get_mut(&token) {
            if endpoint.can_resume_send() {
                match try!(endpoint.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(token),
                    _ => {}
                }
            }
        }

        Ok(self.is_finished_or_keep(sent, sending, endpoints))
    }
}

pub struct MulticastSendingStrategy;

impl MulticastSendingStrategy {
    fn is_finished_or_keep(&self, sent: bool, endpoints: &mut EndpointHashMap) -> (bool, bool) {
        if sent {
            let total_count = endpoints.len();
            let done_count = endpoints.values().filter(|p| p.is_send_finished()).count();
            let finished = total_count == done_count;

            (finished, !finished)
        } else {
            (false, true)
        }
    }
}

impl MsgSendingStrategy for MulticastSendingStrategy {
    fn send(&self, msg: Rc<Message>, endpoints: &mut EndpointHashMap) -> (bool, bool) {
        let mut sent = false;

        for (_, endpoint) in endpoints.into_iter() {
            match endpoint.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                _ => continue
            }
        }

        self.is_finished_or_keep(sent, endpoints)
    }

    fn sent_by(&self, _: mio::Token, endpoints: &mut EndpointHashMap) -> (bool, bool) {
        self.is_finished_or_keep(true, endpoints)
     }

    fn resume_send(&self, msg: &Rc<Message>, token: mio::Token, endpoints: &mut EndpointHashMap) -> io::Result<(bool, bool)> {
        let mut sent = false;

        if let Some(endpoint) = endpoints.get_mut(&token) {
            if endpoint.can_resume_send() {
                match try!(endpoint.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    _ => {}
                }
            }
        }

        Ok(self.is_finished_or_keep(sent, endpoints))
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
        endpoints: &mut EndpointHashMap) {

        self.cancel_timeout = Some(cancel_timeout);

        let msg = Rc::new(msg);
        let (finished, keep) = self.send_to(msg.clone(), token, endpoints);

        if finished {
            self.on_msg_send_finished_ok(event_loop, endpoints);
        } else if keep {
            self.pending_send = Some(msg);
        }
    }

    fn send_to(&self, msg: Rc<Message>, token: mio::Token, endpoints: &mut EndpointHashMap) -> (bool, bool) {
        let mut sent = false;
        let mut sending = None;

        if let Some(endpoint) = endpoints.get_mut(&token) {
            match endpoint.send(msg) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = Some(endpoint.token()),
                _ => {}
            }
        }

        self.is_finished_or_keep(sent, sending, endpoints)
    }

    fn is_finished_or_keep(&self, sent: bool, sending: Option<mio::Token>, endpoints: &mut EndpointHashMap) -> (bool, bool) {
        if let Some(token) = sending {
            self.on_msg_send_started(token, endpoints);

            (false, false)
        } else {
            (sent, !sent)
        }
    }

    fn on_msg_send_started(&self, token: mio::Token, endpoints: &mut EndpointHashMap) {
        let filter_other = |p: &&mut Endpoint| p.token() != token;
        let mut other_endpoints = endpoints.iter_mut().map(|(_, p)| p).filter(filter_other);

        while let Some(endpoint) = other_endpoints.next() {
            endpoint.discard_send(); 
        }
    }

    pub fn sent_by(&mut self,
        event_loop: &mut EventLoop, 
        _: mio::Token,
        endpoints: &mut EndpointHashMap) {

        self.on_msg_send_finished_ok(event_loop, endpoints);
    }

    pub fn resume_send(&mut self,
        event_loop: &mut EventLoop,
        token: mio::Token,
        endpoints: &mut EndpointHashMap) -> io::Result<()> {

        if self.pending_send.is_none() {
            return Ok(());
        }

        let msg = self.pending_send.take().unwrap();
        let (finished, keep) = try!(self.resume_send_to(&msg, token, endpoints));

        if finished {
            self.on_msg_send_finished_ok(event_loop, endpoints);
        } else if keep {
            self.pending_send = Some(msg);
        }

        Ok(())
    }

    fn resume_send_to(&self, msg: &Rc<Message>, token: mio::Token, endpoints: &mut EndpointHashMap) -> io::Result<(bool, bool)> {
        let mut sent = false;
        let mut sending = None;

        if let Some(endpoint) = endpoints.get_mut(&token) {
            if endpoint.can_resume_send() {
                match try!(endpoint.send(msg.clone())) {
                    SendStatus::Completed  => sent = true,
                    SendStatus::InProgress => sending = Some(token),
                    _ => {}
                }
            }
        }

        Ok(self.is_finished_or_keep(sent, sending, endpoints))
    }

    pub fn on_send_timeout(&mut self, event_loop: &mut EventLoop, endpoints: &mut HashMap<mio::Token, Endpoint>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err, endpoints);
    }

    pub fn on_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoints: &mut EndpointHashMap) {
        self.on_msg_send_finished_err(event_loop, err, endpoints);
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop, endpoints: &mut EndpointHashMap) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);
        for (_, endpoint) in endpoints.iter_mut() {
            endpoint.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoints: &mut EndpointHashMap) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));
        for (_, endpoint) in endpoints.iter_mut() {
            endpoint.cancel_send();
        }
    }

    fn on_msg_send_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
        
        self.pending_send = None;
    }

}

pub struct UnaryMsgSender {
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_send: Option<Rc<Message>>
}

impl UnaryMsgSender {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> UnaryMsgSender {
        UnaryMsgSender {
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_send: None
        }
    }

    pub fn send(&mut self,
        event_loop: &mut EventLoop,
        msg: Message,
        cancel_timeout: EventLoopAction,
        endpoint_cell: Option<&mut Endpoint>) {

        self.cancel_timeout = Some(cancel_timeout);

        if endpoint_cell.is_none() {
            return self.pending_send = Some(Rc::new(msg));
        }

        let mut endpoint = endpoint_cell.unwrap();
        let mut sent = false;
        let mut sending = false;
        let msg = Rc::new(msg);

        match endpoint.send(msg.clone()) {
            Ok(SendStatus::Completed)  => sent = true,
            Ok(SendStatus::InProgress) => sending = true,
            _ => {}
        }

        self.pending_send = Some(msg);
        self.process_send_result(event_loop, sent, sending, Some(endpoint));
    }

    pub fn on_send_timeout(&mut self, event_loop: &mut EventLoop, endpoint_cell: Option<&mut Endpoint>) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_finished_err(event_loop, err, endpoint_cell);
    }

    pub fn sent_by(&mut self,
        event_loop: &mut EventLoop, 
        _: mio::Token,
        endpoint_cell: Option<&mut Endpoint>) {

        self.on_msg_send_finished_ok(event_loop, endpoint_cell);
    }

    pub fn resume_send(&mut self,
        event_loop: &mut EventLoop,
        _: mio::Token,
        endpoint_cell: Option<&mut Endpoint>) -> io::Result<()> {

        if self.pending_send.is_none() {
            return Ok(());
        }

        let mut endpoint = endpoint_cell.unwrap();
        let mut sent = false;
        let mut sending = false;
        let msg = self.pending_send.take().unwrap();

        if endpoint.can_resume_send() {
            match endpoint.send(msg.clone()) {
                Ok(SendStatus::Completed)  => sent = true,
                Ok(SendStatus::InProgress) => sending = true,
                _ => {}
            }
        }

        self.pending_send = Some(msg);
        self.process_send_result(event_loop, sent, sending, Some(endpoint));

        Ok(())
    }

    pub fn on_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoint_cell: Option<&mut Endpoint>) {
        self.on_msg_send_finished_err(event_loop, err, endpoint_cell);
    }

    fn process_send_result(&mut self, event_loop: &mut EventLoop, sent: bool, sending: bool, endpoint_cell: Option<&mut Endpoint>) {
        if sent {
            self.on_msg_send_finished_ok(event_loop, endpoint_cell);
        } else if sending {
            self.on_msg_send_started();
        }
    }

    fn on_msg_send_started(&mut self) {
        self.pending_send = None;
    }

    fn on_msg_send_finished_ok(&mut self, event_loop: &mut EventLoop, endpoint_cell: Option<&mut Endpoint>) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgSent);

        if let Some(endpoint) = endpoint_cell {
            endpoint.finish_send(); 
        }
    }

    fn on_msg_send_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, endpoint_cell: Option<&mut Endpoint>) {
        self.on_msg_send_finished(event_loop, SocketEvt::MsgNotSent(err));

        if let Some(endpoint) = endpoint_cell {
            endpoint.cancel_send(); 
        }
    }

    fn on_msg_send_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
        
        self.pending_send = None;
    }
}
