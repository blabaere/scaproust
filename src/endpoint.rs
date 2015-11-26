// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use mio;

use pipe::*;
use EventLoop;
use Message;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OperationStatus {
    Completed,  // done
    InProgress, // ongoing
    Postponed,  // tried, but would have blocked
    Failed,     // tried and failed
    Discarded   // was asked to give up
}

fn can_resume_operation(status: Option<OperationStatus>) -> bool {
    status.map_or_else(|| true, |os| os == OperationStatus::Postponed)
}

fn is_operation_finished(status: Option<OperationStatus>) -> bool {
    match status {
        Some(OperationStatus::Completed) => true,
        Some(OperationStatus::Failed)    => true,
        Some(OperationStatus::Discarded) => true,
        None => false,
        _    => false
    }
}

// A endpoint is responsible for keeping track of the send & recv operation progress of a pipe.
// It is used to link a protocol to a pipe.
pub struct Endpoint {
    token: mio::Token,
    pipe: Pipe,
    send_status: Option<OperationStatus>,
    recv_status: Option<OperationStatus>
}

impl Endpoint {
    pub fn new(token: mio::Token, pipe: Pipe) -> Endpoint {
        Endpoint { 
            token: token,
            pipe: pipe,
            send_status: None,
            recv_status: None
        }
    }

    pub fn token(&self) -> mio::Token {
        self.token
    }

    pub fn send(&mut self, msg: Rc<Message>) -> io::Result<SendStatus> {
        let result = match self.pipe.send(msg) {
            Ok(SendStatus::Completed) => {
                self.send_status = Some(OperationStatus::Completed);
                Ok(SendStatus::Completed)
            },
            Ok(SendStatus::InProgress) => {
                self.send_status = Some(OperationStatus::InProgress);
                Ok(SendStatus::InProgress)
            },
            Ok(SendStatus::Postponed(msg)) => {
                self.send_status = Some(OperationStatus::Postponed);
                Ok(SendStatus::Postponed(msg))
            }
            Err(e) => {
                self.send_status = Some(OperationStatus::Failed);
                Err(e)
            }
        };

        result
    }

    pub fn can_resume_send(&self) -> bool {
        can_resume_operation(self.send_status)
    }

    pub fn is_send_finished(&self) -> bool {
        is_operation_finished(self.send_status)
    }

    pub fn finish_send(&mut self) {
        self.send_status = None;
    }

    pub fn cancel_send(&mut self) {
        self.send_status = None;
        self.pipe.cancel_sending();
    }

    pub fn discard_send(&mut self) {
        self.send_status = Some(OperationStatus::Discarded);
    }

    pub fn recv(&mut self) -> io::Result<RecvStatus> {
        let result = match self.pipe.recv() {
            Ok(RecvStatus::Completed(msg)) => {
                self.recv_status = Some(OperationStatus::Completed);
                Ok(RecvStatus::Completed(msg))
            },
            Ok(RecvStatus::InProgress) => {
                self.recv_status = Some(OperationStatus::InProgress);
                Ok(RecvStatus::InProgress)
            },
            Ok(RecvStatus::Postponed) => {
                self.recv_status = Some(OperationStatus::Postponed);
                Ok(RecvStatus::Postponed)
            }
            Err(e) => {
                self.recv_status = Some(OperationStatus::Failed);
                Err(e)
            }
        };

        result
    }

    pub fn can_resume_recv(&self) -> bool {
        can_resume_operation(self.recv_status)
    }

    pub fn finish_recv(&mut self) {
        self.recv_status = None;
    }

    pub fn cancel_recv(&mut self) {
        self.recv_status = None;
        self.pipe.cancel_receiving();
    }

    pub fn discard_recv(&mut self) {
        self.recv_status = Some(OperationStatus::Discarded);
    }

    pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        self.pipe.ready(event_loop, events);
        /*let (sent, received) = try!(self.pipe.ready(event_loop, events));

        if sent {
            self.send_status = Some(OperationStatus::Completed);
        }

        if received.is_some() {
            self.recv_status = Some(OperationStatus::Completed);
        }

        Ok((sent, received))*/
        // TODO send a notifymsg on the event loop to tell about operation success or failure
    }

    pub fn ready_tx(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        self.pipe.ready(event_loop, events);
        /*let (sent, _) = try!(self.ready(event_loop, events));

        Ok(sent)*/
    }

    pub fn ready_rx(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) {
        self.pipe.ready(event_loop, events);
        /*let (_, received) = try!(self.ready(event_loop, events));

        Ok(received)*/
    }

    pub fn remove(self) -> Pipe {
        self.pipe
    }
}