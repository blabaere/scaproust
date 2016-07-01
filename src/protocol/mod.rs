// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::sync::mpsc;
use std::io;

use mio;

use global::{ SocketType, SocketId, invalid_input_io_error };
use event_loop_msg::{ SocketNotify, SocketOption, PipeEvtSignal };
use transport::pipe::Pipe;
use EventLoop;
use Message;

mod policy;

pub mod push;
pub mod pull;
pub mod pair;
pub mod req;
pub mod rep;
pub mod pbu;
pub mod sub;
pub mod surv;
pub mod resp;
pub mod bus;

pub fn create_protocol(socket_id: SocketId, socket_type: SocketType, evt_tx: Rc<mpsc::Sender<SocketNotify>>) -> Box<Protocol> {
    match socket_type {
        SocketType::Push       => box push::Push::new(socket_id, evt_tx),
        SocketType::Pull       => box pull::Pull::new(socket_id, evt_tx),
        SocketType::Pair       => box pair::Pair::new(socket_id, evt_tx),
        SocketType::Req        => box req::Req::new(socket_id, evt_tx),
        SocketType::Rep        => box rep::Rep::new(socket_id, evt_tx),
        SocketType::Pub        => box pbu::Pub::new(socket_id, evt_tx),
        SocketType::Sub        => box sub::Sub::new(socket_id, evt_tx),
        SocketType::Bus        => box bus::Bus::new(socket_id, evt_tx),
        SocketType::Surveyor   => box surv::Surv::new(socket_id, evt_tx),
        SocketType::Respondent => box resp::Resp::new(socket_id, evt_tx)
    }
}

pub trait Protocol {

/*****************************************************************************/
/*                                                                           */
/* IDENTIFICATION                                                            */
/*                                                                           */
/*****************************************************************************/
    fn get_type(&self) -> SocketType;

    fn id(&self) -> u16 {
        self.get_type().id()
    }
    fn peer_id(&self) -> u16 {
        self.get_type().peer().id()
    }

/*****************************************************************************/
/*                                                                           */
/* READINESS CALLBACK                                                        */
/*                                                                           */
/*****************************************************************************/
    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet);

/*****************************************************************************/
/*                                                                           */
/* PIPE INSERTION AND REMOVAL                                                */
/*                                                                           */
/*****************************************************************************/
    fn add_pipe(&mut self, token: mio::Token, pipe: Pipe, ) -> io::Result<()>;
    fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe>;
    fn open_pipe(&mut self, event_loop: &mut EventLoop, token: mio::Token);

/*****************************************************************************/
/*                                                                           */
/* PIPE EVENT HANDLING                                                       */
/*                                                                           */
/*****************************************************************************/
    fn on_pipe_evt(&mut self, event_loop: &mut EventLoop, tok: mio::Token, evt: PipeEvtSignal) {
        match evt {
            PipeEvtSignal::Opened        => self.on_pipe_opened(event_loop, tok),
            PipeEvtSignal::RecvDone(msg) => self.on_recv_done(event_loop, tok, msg),
            PipeEvtSignal::RecvBlocked   => self.on_recv_blocked(event_loop, tok),
            PipeEvtSignal::SendDone      => self.on_send_done(event_loop, tok),
            PipeEvtSignal::SendBlocked   => self.on_send_blocked(event_loop, tok),
            PipeEvtSignal::Error(_)      |
            PipeEvtSignal::Closed        => {}
        }
    }

    fn on_pipe_opened(&mut self, event_loop: &mut EventLoop, token: mio::Token);

/*****************************************************************************/
/*                                                                           */
/* SEND RELATED METHODS                                                      */
/*                                                                           */
/*****************************************************************************/
    fn send(&mut self, event_loop: &mut EventLoop, msg: Message, timeout_handle: Option<mio::Timeout>);
    fn on_send_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token);
    fn on_send_blocked(&mut self, _: &mut EventLoop, _: mio::Token) {}
    fn on_send_timeout(&mut self, event_loop: &mut EventLoop);
    fn has_pending_send(&self) -> bool;

/*****************************************************************************/
/*                                                                           */
/* RECV RELATED METHODS                                                      */
/*                                                                           */
/*****************************************************************************/
    fn can_recv(&self) -> bool { false }
    fn recv(&mut self, event_loop: &mut EventLoop, timeout_handle: Option<mio::Timeout>);
    fn on_recv_done(&mut self, event_loop: &mut EventLoop, tok: mio::Token, msg: Message);
    fn on_recv_blocked(&mut self, _: &mut EventLoop, _: mio::Token) {}
    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop);

/*****************************************************************************/
/*                                                                           */
/* OPTIONS AND PROTOCOL SPECIFIC METHODS                                     */
/*                                                                           */
/*****************************************************************************/
    fn set_option(&mut self, _: &mut EventLoop, option: SocketOption) -> io::Result<()> {
        match option {
            _ => Err(invalid_input_io_error("option not supported by protocol"))
        }
    }

    fn set_device_item(&mut self, _: bool) -> io::Result<()> {
        Ok(())
    }

    fn on_survey_timeout(&mut self, _: &mut EventLoop) {}
    fn resend(&mut self, _: &mut EventLoop) {}

    fn destroy(&mut self, event_loop: &mut EventLoop);
}
