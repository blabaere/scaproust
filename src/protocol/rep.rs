// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use std::io;

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

use super::sender::*;
use super::receiver::*;

pub struct Rep {
    pipes: HashMap<mio::Token, Pipe>,
    msg_sender: ReturnSender,
    msg_receiver: PolyadicMsgReceiver,
    codec: Codec
}

impl Rep {
    pub fn new(evt_tx: Rc<Sender<SocketEvt>>) -> Rep {
        Rep { 
            pipes: HashMap::new(),
            msg_sender: new_return_sender(evt_tx.clone()),
            msg_receiver: PolyadicMsgReceiver::new(evt_tx.clone()),
            codec: Codec::new()
        }
    }

    /*fn on_msg_send_finished(&mut self, event_loop: &mut EventLoop, evt: SocketEvt) {
        self.backtrace.clear();
    }*/
}

impl Protocol for Rep {
    fn id(&self) -> u16 {
        SocketType::Rep.id()
    }

    fn peer_id(&self) -> u16 {
        SocketType::Req.id()
    }

    fn add_endpoint(&mut self, token: mio::Token, endpoint: Endpoint) {
        self.pipes.insert(token, Pipe::new(token, endpoint));
    }

    fn remove_endpoint(&mut self, token: mio::Token) -> Option<Endpoint> {
        self.pipes.remove(&token).map(|p| p.remove())
    }

    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: EventLoopAction) {
        match self.codec.encode(msg) {
            Err(e) => self.msg_sender.on_send_err(event_loop, e, &mut self.pipes),
            Ok((raw_msg, token)) => self.msg_sender.send(event_loop, raw_msg, cancel_timeout, token, &mut self.pipes)
        };
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_sender.on_send_timeout(event_loop, &mut self.pipes);
    }

    fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: EventLoopAction) {
        self.msg_receiver.recv(event_loop, &mut self.codec, cancel_timeout, &mut self.pipes);
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
        self.msg_receiver.on_recv_timeout(event_loop, &mut self.pipes)
    }

/******************************************************************************
THIS IS NOT GOOD !!!
RESUME SEND SHOULD ONLY BE CALLED IF THE READIED PIPE 
IS THE ONE THAT SENT THE REQUEST
******************************************************************************/
    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
        let mut sent = false;
        let mut received = None;

        if let Some(pipe) = self.pipes.get_mut(&token) {
            let (s, r) = try!(pipe.ready(event_loop, events));
            sent = s;
            received = r;
        }

        let send_result = match sent {
            true  => Ok(self.msg_sender.sent_by(event_loop, token, &mut self.pipes)),
            false => self.msg_sender.resume_send(event_loop, token, &mut self.pipes)
        };

        let recv_result = match received {
            Some(msg) => Ok(self.msg_receiver.received_by(event_loop, &mut self.codec, msg, token, &mut self.pipes)),
            None => self.msg_receiver.resume_recv(event_loop, &mut self.codec, token, &mut self.pipes)
        };

        send_result.and(recv_result)
    }
}

struct Codec {
    backtrace: Vec<u8>,
    ttl: u8
}

impl Codec {
    fn new() -> Codec {
        Codec {
            backtrace: Vec::with_capacity(64),
            ttl: 8
        }
    }

    fn encode(&mut self, msg: Message)  -> io::Result<(Message, mio::Token)> {
        let (header, body) = msg.explode();
        let pipe_token = try!(self.restore_pipe_id_from_backtrace());
        let header = try!(self.restore_header_from_backtrace(header));

        Ok((Message::with_header_and_body(header, body), pipe_token))
    }

    fn restore_pipe_id_from_backtrace(&self) -> io::Result<mio::Token> {
        if self.backtrace.len() < 4 {
            return Err(other_io_error("no backtrace from received message"));
        }
        let pipe_id_bytes = vec!(
            self.backtrace[0],
            self.backtrace[1],
            self.backtrace[2],
            self.backtrace[3]
        );
        let mut pipe_id_reader = io::Cursor::new(pipe_id_bytes);
        let pipe_id = try!(pipe_id_reader.read_u32::<BigEndian>());
        let pipe_token = mio::Token(pipe_id as usize);

        Ok(pipe_token)
    }

    fn restore_header_from_backtrace(&self, mut header: Vec<u8>) -> io::Result<Vec<u8>> {
        if self.backtrace.len() < 8 {
            return Err(other_io_error("no header in backtrace from received message"));
        }

        let backtrace = &self.backtrace[4..];
        
        header.reserve(backtrace.len());
        header.push_all(backtrace);

        Ok(header)
    }

    fn save_received_header_to_backtrace(&mut self, msg: &Message) {
        self.backtrace.clear();
        self.backtrace.push_all(msg.get_header());
    }
}

impl MsgDecoder for Codec {
    fn decode(&mut self, raw_msg: Message, pipe_token: mio::Token) -> io::Result<Option<Message>> {
        let (mut header, mut body) = raw_msg.explode();
        let pipe_id = pipe_token.as_usize() as u32;

        header.reserve(4);
        try!(header.write_u32::<BigEndian>(pipe_id));

        let mut hops = 0;
        loop {
            if hops >= self.ttl {
                return Ok(None);
            }

            hops += 1;

            if body.len() < 4 {
                return Ok(None);
            }

            let tail = body.split_off(4);
            header.push_all(&body);

            let position = header.len() - 4;
            if header[position] & 0x80 != 0 {
                let msg = Message::with_header_and_body(header, tail);

                self.save_received_header_to_backtrace(&msg);
                return Ok(Some(msg));
            }
            body = tail;
        }
    }
}
