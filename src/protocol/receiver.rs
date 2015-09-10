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

pub trait MsgDecoder {
    fn decode(&mut self, msg: Message, pipe_token: mio::Token) -> io::Result<Option<Message>>;
}

pub struct NoopMsgDecoder;

impl MsgDecoder for NoopMsgDecoder {
    fn decode(&mut self, msg: Message, _: mio::Token) -> io::Result<Option<Message>> {
        Ok(Some(msg))
    }
}

pub struct PolyadicMsgReceiver {
    evt_sender: Rc<mpsc::Sender<SocketEvt>>,
    cancel_timeout: Option<EventLoopAction>,
    pending_recv: bool
}

impl PolyadicMsgReceiver {
    pub fn new(evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> PolyadicMsgReceiver {
        PolyadicMsgReceiver {
            evt_sender: evt_tx,
            cancel_timeout: None,
            pending_recv: false
        }
    }

    pub fn recv(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        cancel_timeout: EventLoopAction,
        pipes: &mut PipeHashMap) {

        self.cancel_timeout = Some(cancel_timeout);

        let mut received = None;
        let mut receiving = None;
        for (_, pipe) in pipes.iter_mut() {
            match pipe.recv() {
                Ok(RecvStatus::Completed(msg)) => received = Some((msg, pipe.token())),
                Ok(RecvStatus::InProgress)     => receiving = Some(pipe.token()),
                _ => continue
            }
            break;
        }

        self.pending_recv = true;
        self.process_recv_result(event_loop, decoder, received, receiving, pipes);
    }

    pub fn received_by(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        raw_msg: Message, 
        token: mio::Token,
        pipes: &mut PipeHashMap) {

        self.process_recv_result(event_loop, decoder, Some((raw_msg, token)), None, pipes);
    }

    pub fn resume_recv(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        token: mio::Token,
        pipes: &mut PipeHashMap) -> io::Result<()> {

        if self.pending_recv == false {
            return Ok(());
        }

        let mut received = None;
        let mut receiving = None;

        if let Some(pipe) = pipes.get_mut(&token) {
            if pipe.can_resume_recv() {
                match try!(pipe.recv()) {
                    RecvStatus::Completed(msg) => received = Some((msg, pipe.token())),
                    RecvStatus::InProgress     => receiving = Some(token),
                    _ => {}
                }
            }
        }

        self.process_recv_result(event_loop, decoder, received, receiving, pipes);

        Ok(())
    }

    pub fn on_recv_timeout(&mut self, event_loop: &mut EventLoop, pipes: &mut PipeHashMap) {
        let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

        self.on_msg_recv_finished_err(event_loop, err, pipes);
    }

    fn process_recv_result(&mut self,
        event_loop: &mut EventLoop,
        decoder: &mut MsgDecoder,
        received: Option<(Message, mio::Token)>,
        receiving: Option<mio::Token>,
        pipes: &mut PipeHashMap) {

        if let Some((raw_msg, tok)) = received {
            match decoder.decode(raw_msg, tok) {
                Err(e)        => self.on_msg_recv_finished_err(event_loop, e, pipes),
                Ok(Some(msg)) => self.on_msg_recv_finished_ok(event_loop, msg, pipes),
                Ok(None)      => {
                /* 
                The raw msg has been decoded, but it does not match the current interest.
                Here the recv operation should probably be started again on the pipe, 
                otherwise we won't receive anymore during this operation.
                The correct way is probably to set the pipe's recv operation status flag to PostPoned or None
                */
                }
            }
        }
        else if let Some(token) = receiving {
            self.on_msg_recv_started(token, pipes);
        }
    }

    fn on_msg_recv_finished_ok(&mut self, event_loop: &mut EventLoop, msg: Message, pipes: &mut PipeHashMap) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgRecv(msg));

        for (_, pipe) in pipes.iter_mut() {
            pipe.finish_recv(); 
        }
    }

    fn on_msg_recv_finished_err(&mut self, event_loop: &mut EventLoop, err: io::Error, pipes: &mut PipeHashMap) {
        self.on_msg_recv_finished(event_loop, SocketEvt::MsgNotRecv(err));

        for (_, pipe) in pipes.iter_mut() {
            pipe.cancel_recv(); 
        }
    }

    fn on_msg_recv_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        let _ = self.evt_sender.send(evt);
        let timeout = self.cancel_timeout.take();

        timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));

        self.pending_recv = false;
    }

    fn on_msg_recv_started(&mut self, token: mio::Token, pipes: &mut PipeHashMap) {
        self.pending_recv = false;
        for (_, pipe) in pipes.iter_mut() {
            if pipe.token() == token {
                continue;
            } else {
                pipe.discard_recv();
            }
        }
    }
}