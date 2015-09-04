// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use time;

use mio;

use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };

use super::Protocol;
use pipe::*;
use endpoint::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use EventLoopAction;
use Message;

pub struct Surv {
    pipes: HashMap<mio::Token, Pipe>,
    evt_sender: Rc<Sender<SocketEvt>>,
    cancel_send_timeout: Option<EventLoopAction>,
    cancel_recv_timeout: Option<EventLoopAction>,
    pending_survey_id: Option<u32>,
    survey_id_seq: u32
}

impl Surv {
    pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Surv {
        Surv { 
            pipes: HashMap::new(),
            evt_sender: evt_tx,
            cancel_send_timeout: None,
            cancel_recv_timeout: None,
            pending_survey_id: None,
            survey_id_seq: time::get_time().nsec as u32
        }
    }

    fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
        let evt = SocketEvt::MsgSent;
        let timeout = self.cancel_send_timeout.take();

        self.on_msg_sending_finished();
        self.send_event_and_cancel_timeout(event_loop, evt, timeout)
    }

    fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        let evt = SocketEvt::MsgNotSent(err);
        let timeout = self.cancel_send_timeout.take();

        self.on_msg_sending_finished();
        self.send_event_and_cancel_timeout(event_loop, evt, timeout)
    }

    fn on_msg_sending_finished(&mut self) {
        for (_, pipe) in self.pipes.iter_mut() {
            pipe.on_msg_sending_finished();
        }
    }

    fn on_msg_recv_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
        let evt = SocketEvt::MsgRecv(msg);
        let timeout = self.cancel_recv_timeout.take();

        self.send_event_and_cancel_timeout(event_loop, evt, timeout)
    }

    fn on_msg_recv_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
        let evt = SocketEvt::MsgNotRecv(err);
        let timeout = self.cancel_recv_timeout.take();

        self.send_event_and_cancel_timeout(event_loop, evt, timeout)
    }

    fn send_event_and_cancel_timeout(&self, 
        event_loop: &mut EventLoop, 
        evt: SocketEvt, 
        timeout: Option<EventLoopAction>) {
        
        let _ = self.evt_sender.send(evt);

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
    }

    fn next_survey_id(&mut self) -> u32 {
        let next_id = self.survey_id_seq | 0x80000000;

        self.survey_id_seq += 1;

        next_id
    }

    fn raw_msg_to_msg(&mut self, raw_msg: Message) -> io::Result<(Message, u32)> {
        let (mut header, mut payload) = raw_msg.explode();
        let body = payload.split_off(4);
        let mut req_id_reader = io::Cursor::new(payload);

        let req_id = try!(req_id_reader.read_u32::<BigEndian>());

        if header.len() == 0 {
            header = req_id_reader.into_inner();
        } else {
            let req_id_bytes = req_id_reader.into_inner();
            header.push_all(&req_id_bytes);
        }

        Ok((Message::with_header_and_body(header, body), req_id))
    }

    fn msg_to_raw_msg(&mut self, msg: Message, req_id: u32) -> io::Result<Message> {
        let mut raw_msg = msg;

        raw_msg.header.reserve(4);
        try!(raw_msg.header.write_u32::<BigEndian>(req_id));

        Ok(raw_msg)
    }

    fn on_raw_msg_recv(&mut self, event_loop: &mut EventLoop, raw_msg: Message) {
        if raw_msg.get_body().len() < 4 {
            return;
        }

        match self.raw_msg_to_msg(raw_msg) {
            Ok((msg, survey_id)) => {
                if self.pending_survey_id == Some(survey_id) {
                    self.on_msg_recv_ok(event_loop, msg);
                }
            },
            Err(e) => {
                self.on_msg_recv_err(event_loop, e);
            }
        }
    }
}

impl Protocol for Surv {
    fn id(&self) -> u16 {
        SocketType::Surveyor.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Respondent.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut result = Ok(());
        let mut sent_msg = false;
        let mut send_msg_error = false;
        let mut received_msg = None;
        let mut receiving_msg = false;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            match pipe.ready(event_loop, events) {
                Ok((sent, received))  => {

                    if sent {
                        sent_msg = true;
                    } else {
                        match pipe.resume_pending_send() {
                            Ok(Some(true)) => sent_msg = true,
                            Err(e)         =>  {
                                send_msg_error = true;
                                result = Err(e);
                            },
                            _              => {}
                        }
                    }

                    if received.is_some() {
                        received_msg = received;
                    } else {
                        match pipe.resume_pending_recv() {
                            Ok(Some(RecvStatus::Completed(msg))) => received_msg = Some(msg),
                            Ok(Some(RecvStatus::InProgress))     => receiving_msg = true,
                            Ok(Some(RecvStatus::Postponed))      => {},
                            Ok(None)                             => {},
                            Err(e)                               => result = Err(e)
                        }
                    }
                },
                Err(e)    => {
                    send_msg_error = true;
                    result = Err(e);
                }
            }
        }

        let mut notify_sent = false;
        if sent_msg | send_msg_error {
            let mut pipe_count = 0;
            let mut pipe_done_count = 0;

            for (_, pipe) in self.pipes.iter_mut() {
                match pipe.send_status() {
                    Some(true) => {
                        pipe_count += 1;
                        pipe_done_count += 1;
                    },
                    Some(false) => {
                        pipe_count += 1;
                    }
                    None => continue
                }

                notify_sent = pipe_count > 0 && pipe_count == pipe_done_count;
            }

            if notify_sent {
                for (_, pipe) in self.pipes.iter_mut() {
                    pipe.reset_pending_send();
                }
                self.on_msg_send_ok(event_loop);
            }
        }

        if received_msg.is_some() | receiving_msg {
            for (_, pipe) in self.pipes.iter_mut() {
                pipe.reset_pending_recv();
            }
        }

        if received_msg.is_some() && self.pending_survey_id.is_some() {
            let raw_msg = received_msg.unwrap();

            self.on_raw_msg_recv(event_loop, raw_msg);
        }

        result
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        let survey_id = self.next_survey_id();

        self.cancel_send_timeout = Some(cancel_timeout);
        self.pending_survey_id = Some(survey_id);

        let raw_msg = match self.msg_to_raw_msg(msg, survey_id) {
            Err(e) => {
                self.on_msg_send_err(event_loop, e);
                return;
            },
            Ok(raw_msg) => raw_msg
        };

        let mut sent_count = 0;
        let mut sending_count = 0;
        let mut pending_count = 0;
        let shared_msg = Rc::new(raw_msg);

        for (_, pipe) in self.pipes.iter_mut() {
            match pipe.send(shared_msg.clone()) {
                Ok(Some(true))  => sent_count    += 1,
                Ok(Some(false)) => sending_count += 1,
                Ok(None)        => pending_count += 1,
                Err(_)          => {}
                // this pipe looks dead, but it will be taken care of during next ready notification
            }
        }

        let sent = sent_count == self.pipes.len();
        let sending = sending_count > 0;
        let pending = pending_count > 0;

        if sent {
            self.on_msg_send_ok(event_loop);
        }

        if sent {
            for (_, pipe) in self.pipes.iter_mut() {
                pipe.reset_pending_send();
            }
        } else if pending == false && sending == false {
            let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

            self.on_msg_send_err(event_loop, err);
        }
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

        self.on_msg_send_err(event_loop, err);

        for (_, pipe) in self.pipes.iter_mut() {
            pipe.on_send_timeout();
        }
    }

    // TODO do not cancel any pending recv, since we must receive a response from all peers ...
    // This stuff should look like pub sending
    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.cancel_recv_timeout = Some(cancel_timeout);

        let mut received = None;
        let mut receiving = false;
        let mut pending = false;

        for (_, pipe) in self.pipes.iter_mut() {
            match pipe.recv() {
                Ok(RecvStatus::Completed(msg)) => received = Some(msg),
                Ok(RecvStatus::InProgress)     => receiving = true,
                Ok(RecvStatus::Postponed)      => pending = true,
                Err(_)                         => continue
            }

            if received.is_some() | receiving {
                break;
            }
        }

        if received.is_some() | receiving {
            for (_, pipe) in self.pipes.iter_mut() {
                pipe.reset_pending_recv();
            }
        } else if pending == false {
            let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

            self.on_msg_recv_err(event_loop, err);
        }

        if received.is_some() && self.pending_survey_id.is_some() {
            let raw_msg = received.unwrap();

            self.on_raw_msg_recv(event_loop, raw_msg);
        }
    }
    
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_err(event_loop, err);

        for (_, pipe) in self.pipes.iter_mut() {
            pipe.on_recv_timeout();
        }
    }
}
