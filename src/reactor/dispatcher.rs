// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;
use std::io;

use mio::{Token, Poll, Ready, PollOpt};
use mio::timer::{Timer};
use mio::channel::{Receiver};

use core::{SocketId, EndpointId, session, socket, endpoint, context};
use transport::{pipe, acceptor};
use super::{Signal, Request};
use super::event_loop::{EventLoop, EventHandler};
use super::bus::EventLoopBus;
use super::adapter::{EndpointCollection, Schedule, SocketEventLoopContext};
use sequence::Sequence;

const CHANNEL_TOKEN: Token = Token(::std::usize::MAX - 1);
const BUS_TOKEN: Token     = Token(::std::usize::MAX - 2);
const TIMER_TOKEN: Token   = Token(::std::usize::MAX - 3);

pub struct Dispatcher {
    // request inputs
    channel: Receiver<Request>,
    bus: EventLoopBus<Signal>,
    timer: Timer<context::Schedulable>,

    // request handlers
    sockets: session::Session,
    endpoints: EndpointCollection,
    schedule: Schedule
}

impl Dispatcher {
    pub fn new(rx: Receiver<Request>, tx: Sender<session::Reply>) -> Dispatcher {
        let seq = Sequence::new();

        Dispatcher {
            channel: rx,
            bus: EventLoopBus::new(),
            timer: Timer::default(),
            sockets: session::Session::new(seq.clone(), tx),
            endpoints: EndpointCollection::new(seq.clone()),
            schedule: Schedule::new(seq.clone())
        }
    }

/*****************************************************************************/
/*                                                                           */
/* run event loop                                                            */
/*                                                                           */
/*****************************************************************************/

    pub fn run(&mut self) -> io::Result<()> {
        let mut event_loop = try!(EventLoop::new());
        let interest = Ready::readable();
        let opt = PollOpt::edge();

        try!(event_loop.register(&self.channel, CHANNEL_TOKEN, interest, opt));
        try!(event_loop.register(&self.bus, BUS_TOKEN, interest, opt));
        try!(event_loop.register(&self.timer, TIMER_TOKEN, interest, opt));

        event_loop.run(self)
    }

/*****************************************************************************/
/*                                                                           */
/* retrieves requests from inputs                                            */
/*                                                                           */
/*****************************************************************************/

    fn process_channel(&mut self, el: &mut EventLoop) {
        /*for _ in 0..self.config.messages_per_tick {
            match self.channel.try_recv() {
                Ok(req) => self.dispatcher.process_request(poll, req),
                _ => break,
            }
        }*/
        while let Ok(req) = self.channel.try_recv() {
            self.process_request(el, req);
        }
    }
    fn process_bus(&mut self, el: &mut EventLoop) {
        while let Some(signal) = self.bus.recv() {
            self.process_signal(el, signal);
        }
    }
    fn process_timer(&mut self, el: &mut EventLoop) {
        while let Some(timeout) = self.timer.poll() {
            self.process_tick(el, timeout);
        }
    }

/*****************************************************************************/
/*                                                                           */
/* dispatch requests by input                                                */
/*                                                                           */
/*****************************************************************************/
    fn process_request(&mut self, el: &mut EventLoop, request: Request) {
        match request {
            Request::Session(req) => self.process_session_request(req),
            Request::Socket(id, req) => self.process_socket_request(el, id, req),
            _ => {}
        }
    }
    fn process_signal(&mut self, el: &mut EventLoop, signal: Signal) {
        match signal {
            Signal::PipeCmd(_, eid, cmd)       => self.process_pipe_cmd(el, eid, cmd),
            Signal::AcceptorCmd(_, eid, cmd)   => self.process_acceptor_cmd(el, eid, cmd),
            Signal::PipeEvt(sid, eid, evt)     => self.process_pipe_evt(el, sid, eid, evt),
            Signal::AcceptorEvt(sid, eid, evt) => self.process_acceptor_evt(el, sid, eid, evt),
            Signal::SocketEvt(_, _) => {}
        }
    }
    fn process_tick(&mut self, el: &mut EventLoop, timeout: context::Schedulable) {
        match timeout {
            context::Schedulable::Reconnect(sid, spec) => self.apply_on_socket(el, sid, |socket, ctx| socket.reconnect(ctx, spec)),
            context::Schedulable::Rebind(sid, spec)    => self.apply_on_socket(el, sid, |socket, ctx| socket.rebind(ctx, spec)),
            context::Schedulable::SendTimeout(sid)     => self.apply_on_socket(el, sid, |socket, ctx| socket.on_send_timeout(ctx)),
            context::Schedulable::RecvTimeout(sid)     => self.apply_on_socket(el, sid, |socket, ctx| socket.on_recv_timeout(ctx))
        }
    }
    fn process_io(&mut self, el: &mut EventLoop, token: Token, events: Ready) {
        let eid = EndpointId::from(token);
        {
            if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
                pipe.ready(el, &mut self.bus, events);
                return;
            } 
        }
        {
            if let Some(acceptor) = self.endpoints.get_acceptor_mut(eid) {
                acceptor.ready(el, &mut self.bus, events);
                return;
            }
        }
    }

/*****************************************************************************/
/*                                                                           */
/* process regular requests                                                  */
/*                                                                           */
/*****************************************************************************/
    fn process_session_request(&mut self, request: session::Request) {
        match request {
            session::Request::CreateSocket(ctor) => self.sockets.add_socket(ctor),
            _ => {}
        }
    }
    fn process_socket_request(&mut self, el: &mut EventLoop, id: SocketId, request: socket::Request) {
        match request {
            socket::Request::Connect(url) => self.apply_on_socket(el, id, |socket, ctx| socket.connect(ctx, url)),
            socket::Request::Bind(url)    => self.apply_on_socket(el, id, |socket, ctx| socket.bind(ctx, url)),
            socket::Request::Send(msg)    => self.apply_on_socket(el, id, |socket, ctx| socket.send(ctx, msg)),
            socket::Request::Recv         => self.apply_on_socket(el, id, |socket, ctx| socket.recv(ctx)),
            socket::Request::SetOption(x) => self.apply_on_socket(el, id, |socket, ctx| socket.set_opt(ctx, x))
        }
    }

/*****************************************************************************/
/*                                                                           */
/* process signal requests                                                   */
/*                                                                           */
/*****************************************************************************/
    fn process_pipe_cmd(&mut self, el: &mut EventLoop, eid: EndpointId, cmd: pipe::Command) {
        if let Some(pipe) = self.endpoints.get_pipe_mut(eid) {
            pipe.process(el, &mut self.bus, cmd);
        }
    }
    fn process_acceptor_cmd(&mut self, el: &mut EventLoop, eid: EndpointId, cmd: acceptor::Command) {
        if let Some(acceptor) = self.endpoints.get_acceptor_mut(eid) {
            acceptor.process(el, &mut self.bus, cmd);
        }
    }
    fn process_pipe_evt(&mut self, el: &mut EventLoop, sid: SocketId, eid: EndpointId, evt: pipe::Event) {
        match evt {
            pipe::Event::Opened        => self.apply_on_socket(el, sid, |socket, ctx| socket.on_pipe_opened(ctx, eid)),
            pipe::Event::CanSend       => self.apply_on_socket(el, sid, |socket, ctx| socket.on_send_ready(ctx, eid)),
            pipe::Event::Sent          => self.apply_on_socket(el, sid, |socket, ctx| socket.on_send_ack(ctx, eid)),
            pipe::Event::CanRecv       => self.apply_on_socket(el, sid, |socket, ctx| socket.on_recv_ready(ctx, eid)),
            pipe::Event::Received(msg) => self.apply_on_socket(el, sid, |socket, ctx| socket.on_recv_ack(ctx, eid, msg)),
            pipe::Event::Error(err)    => self.apply_on_socket(el, sid, |socket, ctx| socket.on_pipe_error(ctx, eid, err)),
            pipe::Event::Closed        => self.endpoints.remove_pipe(eid)
        }
    }
    fn process_acceptor_evt(&mut self, el: &mut EventLoop, sid: SocketId, aid: EndpointId, evt: acceptor::Event) {
        match evt {
            // Maybe the controller should be removed from the endpoint collection
            acceptor::Event::Error(e) => self.apply_on_socket(el, sid, |socket, ctx| socket.on_acceptor_error(ctx, aid, e)),
            acceptor::Event::Accepted(pipes) => {
                for pipe in pipes {
                    let pipe_id = self.endpoints.insert_pipe(sid, pipe);

                    self.apply_on_socket(el, sid, |socket, ctx| socket.on_pipe_accepted(ctx, aid, pipe_id));
                }
            },
            _ => {}
        }
    }

/*****************************************************************************/
/*                                                                           */
/* process timed requests                                                    */
/*                                                                           */
/*****************************************************************************/

    fn apply_on_socket<F>(&mut self, event_loop: &mut EventLoop, id: SocketId, f: F) 
    where F : FnOnce(&mut socket::Socket, &mut SocketEventLoopContext) {
        if let Some(socket) = self.sockets.get_socket_mut(id) {
            let mut ctx = SocketEventLoopContext::new(
                id,
                &mut self.bus,
                &mut self.endpoints,
                &mut self.schedule,
                &mut self.timer);

            f(socket, &mut ctx);
        }
    }
}

impl EventHandler for Dispatcher {
    fn handle(&mut self, el: &mut EventLoop, token: Token, events: Ready) {
        if token == CHANNEL_TOKEN {
            return self.process_channel(el);
        }
        if token == BUS_TOKEN {
            return self.process_bus(el);
        }
        if token == TIMER_TOKEN {
            return self.process_timer(el);
        }

        self.process_io(el, token, events)
    }
}
