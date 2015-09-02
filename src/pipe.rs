// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use endpoint::*;
use EventLoop;
use Message;

pub enum SendOperationStatus {
    Completed,
    InProgress,
    Postponed(Rc<Message>),
    Failed
}

// A pipe is responsible for keeping track of the send & recv operation progress of an endpoint.
// It is used to link a protocol to an endpoint.
pub struct Pipe {
    token: mio::Token,
    endpoint: Endpoint,
    send_status: Option<SendOperationStatus>,
    pending_recv: bool
    //recv_status: Option<RecvOperationStatus>
}

impl Pipe {
    pub fn new(token: mio::Token, endpoint: Endpoint) -> Pipe {
        Pipe { 
            token: token,
            endpoint: endpoint,
            send_status: None,
            pending_recv: false
            //recv_status: None
        }
    }

    pub fn token(&self) -> mio::Token {
        self.token
    }

    pub fn send_status(&self) -> Option<bool> {
        match self.send_status {
            None => None,
            Some(SendOperationStatus::Completed)    => Some(true),
            Some(SendOperationStatus::Failed)       => Some(true),
            Some(SendOperationStatus::InProgress)   => Some(false),
            Some(SendOperationStatus::Postponed(_)) => Some(false)
        }
    }

    pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<(bool, Option<Message>)> {
        self.endpoint.ready(event_loop, events)
    }

    pub fn ready_tx(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<bool> {
        let (sent, _) = try!(self.endpoint.ready(event_loop, events));

        Ok(sent)
    }

    pub fn ready_rx(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<Option<Message>> {
        let (_, received) = try!(self.endpoint.ready(event_loop, events));

        Ok(received)
    }

    pub fn send(&mut self, msg: Rc<Message>) -> io::Result<Option<bool>> {
        let result = match self.endpoint.send(msg) {
            Ok(SendStatus::Completed) => {
                self.send_status = Some(SendOperationStatus::Completed);
                Ok(Some(true))
            },
            Ok(SendStatus::InProgress) => {
                self.send_status = Some(SendOperationStatus::InProgress);
                Ok(Some(false))
            },
            Ok(SendStatus::Postponed(message)) => {
                self.send_status = Some(SendOperationStatus::Postponed(message));
                Ok(None)
            }
            Err(e) => {
                self.send_status = Some(SendOperationStatus::Failed);
                Err(e)
            }
        };

        result
    }

    pub fn on_send_timeout(&mut self) {
        self.send_status = None;
        self.endpoint.cancel_sending();
    }

    pub fn resume_pending_send(&mut self) -> io::Result<Option<bool>> {
        match self.send_status.take() {
            Some(SendOperationStatus::Postponed(msg)) => self.send(msg),
            _ => Ok(None)
        }
    }

    pub fn reset_pending_send(&mut self) {
        self.send_status = None;
    }

    pub fn on_msg_sending_finished(&mut self) {
        self.send_status = None;
        self.endpoint.cancel_sending();
        // this is probably useless, the only way for the pipe to require some cleaning
        // would be a message sending in progress, and therefore, not finished
    }

    pub fn recv(&mut self) -> io::Result<RecvStatus> {
        let progress = try!(self.endpoint.recv());

        self.pending_recv = match progress {
            RecvStatus::Completed(_) => false,
            RecvStatus::InProgress   => false,
            RecvStatus::Postponed    => true
        };

        Ok(progress)
    }

    pub fn on_recv_timeout(&mut self) {
        self.pending_recv = false;
        self.endpoint.cancel_receiving();
    }

    pub fn resume_pending_recv(&mut self) -> io::Result<Option<RecvStatus>> {
        let result = if self.pending_recv {
            let status = try!(self.endpoint.recv());
            Some(status)
        } else {
            None
        };

        Ok(result)
    }

    pub fn reset_pending_recv(&mut self) {
        self.pending_recv = false;
    }

    pub fn remove(self) -> Endpoint {
        self.endpoint
    }
}