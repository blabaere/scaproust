// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;
use std::time::Duration;

use super::{BuildIdHasher, SocketId, EndpointId, Message, EndpointTmpl, EndpointSpec, EndpointDesc, Scheduled };
use super::endpoint::{Pipe, Acceptor};
use super::config::{Config, ConfigOption};
use super::context::{Context, Schedulable, Event};
use io_error::*;

pub enum Request {
    Connect(String),
    Bind(String),
    Send(Message, bool),
    Recv(bool),
    SetOption(ConfigOption),
    Close
}

pub enum Reply {
    Err(io::Error),
    Connect(EndpointId),
    Bind(EndpointId),
    Send,
    Recv(Message),
    SetOption
}

pub struct Socket {
    id: SocketId,
    reply_sender: Sender<Reply>,
    protocol: Box<Protocol>,
    pipes: HashMap<EndpointId, Pipe, BuildIdHasher>,
    acceptors: HashMap<EndpointId, Acceptor, BuildIdHasher>,
    config: Config
}

/*****************************************************************************/
/*                                                                           */
/* Protocol                                                                  */
/*                                                                           */
/*****************************************************************************/

pub trait Protocol {
    fn id(&self) -> u16;
    fn peer_id(&self) -> u16;

    fn add_pipe(&mut self, ctx: &mut Context, eid: EndpointId, pipe: Pipe);
    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<Pipe>;

    fn send(&mut self, ctx: &mut Context, msg: Message, timeout: Option<Scheduled>);
    fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId);
    fn on_send_timeout(&mut self, ctx: &mut Context);
    fn on_send_ready(&mut self, ctx: &mut Context, eid: EndpointId);
    fn on_send_not_ready(&mut self, ctx: &mut Context, eid: EndpointId) {}
    
    fn recv(&mut self, ctx: &mut Context, timeout: Option<Scheduled>);
    fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, msg: Message);
    fn on_recv_timeout(&mut self, ctx: &mut Context);
    fn on_recv_ready(&mut self, ctx: &mut Context, eid: EndpointId);
    fn on_recv_not_ready(&mut self, ctx: &mut Context, eid: EndpointId) {}

    fn is_send_ready(&self) -> bool;
    fn is_recv_ready(&self) -> bool;

    fn set_option(&mut self, _: ConfigOption) -> io::Result<()> {
        Err(invalid_input_io_error("option not supported"))
    }
    fn on_timer_tick(&mut self, _: &mut Context, _: Schedulable) {
    }
    fn on_device_plugged(&mut self, _: &mut Context) {}
    fn close(&mut self, ctx: &mut Context);
}

pub type ProtocolCtor = Box<Fn(Sender<Reply>) -> Box<Protocol> + Send>;

/*****************************************************************************/
/*                                                                           */
/* Socket                                                                    */
/*                                                                           */
/*****************************************************************************/

impl Socket {
    pub fn new(id: SocketId, reply_tx: Sender<Reply>, proto: Box<Protocol>) -> Socket {
        Socket {
            id: id,
            reply_sender: reply_tx,
            protocol: proto,
            pipes: HashMap::default(),
            acceptors: HashMap::default(),
            config: Config::default()
        }
    }

    fn get_protocol_ids(&self) -> (u16, u16) {
        let proto_id = self.protocol.id();
        let peer_proto_id = self.protocol.peer_id();

        (proto_id, peer_proto_id)
    }

    fn send_reply(&self, reply: Reply) {
        let _ = self.reply_sender.send(reply);
    }

    pub fn poll(&self, ctx: &mut Context) {
        ctx.raise(Event::CanRecv(self.protocol.is_recv_ready()));
        ctx.raise(Event::CanSend(self.protocol.is_send_ready()));
    }

/*****************************************************************************/
/*                                                                           */
/* endpoint creation                                                         */
/*                                                                           */
/*****************************************************************************/

    fn create_endpoint_desc(&self) -> EndpointDesc {
        EndpointDesc {
            send_priority: self.config.send_priority,
            recv_priority: self.config.recv_priority,
            tcp_no_delay: self.config.tcp_no_delay,
            recv_max_size: self.config.recv_max_size
        }
    }

    fn create_endpoint_spec(&self, url: String) -> EndpointSpec {
        EndpointSpec {
            url: url,
            desc: self.create_endpoint_desc()
        }
    }

    fn create_endpoint_tmpl(&self, url: String) -> EndpointTmpl {
        EndpointTmpl {
            pids: self.get_protocol_ids(),
            spec: self.create_endpoint_spec(url)
        }
    }

/*****************************************************************************/
/*                                                                           */
/* connect                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn connect(&mut self, ctx: &mut Context, url: String) {
        let tmpl = self.create_endpoint_tmpl(url);

        match ctx.connect(self.id, &tmpl) {
            Ok(id) => self.on_connect_success(ctx, id, tmpl.spec),
            Err(e) => self.on_connect_error(e)
        };
    }

    fn on_connect_success(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        let pipe = self.connect_pipe(eid, spec);

        self.insert_pipe(ctx, eid, pipe);
        self.send_reply(Reply::Connect(eid));
    }

    fn on_connect_error(&mut self, err: io::Error) {
        self.send_reply(Reply::Err(err));
    }

    fn schedule_reconnect(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        let task = Schedulable::Reconnect(eid, spec);
        let delay = self.config.retry_ivl;
        let _ = ctx.schedule(task, delay); 
        // TODO maybe we should keep track of the scheduled reconnection
        // In case the facade wants to close the ep somewhere between the error and the timeout
    }

    pub fn reconnect(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        let pids = self.get_protocol_ids();
        let tmpl = EndpointTmpl {
            pids: pids,
            spec: spec
        };

        match ctx.reconnect(self.id, eid, &tmpl) {
            Ok(_)  => self.on_reconnect_success(ctx, eid, tmpl.spec),
            Err(_) => self.on_reconnect_error(ctx, eid, tmpl.spec)
        }
    }

    fn on_reconnect_success(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        self.insert_pipe(ctx, eid, Pipe::from_spec(eid, spec));
    }

    fn on_reconnect_error(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        self.schedule_reconnect(ctx, eid, spec);
    }

/*****************************************************************************/
/*                                                                           */
/* bind                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn bind(&mut self, ctx: &mut Context, url: String) {
        let tmpl = self.create_endpoint_tmpl(url);

        match ctx.bind(self.id, &tmpl) {
            Ok(id) => self.on_bind_success(ctx, id, tmpl.spec),
            Err(e) => self.on_bind_error(e)
        };
    }

    fn on_bind_success(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        let acceptor = self.connect_acceptor(eid, spec);

        acceptor.open(ctx);

        self.acceptors.insert(eid, acceptor);
        self.send_reply(Reply::Bind(eid));
    }

    fn on_bind_error(&mut self, err: io::Error) {
        self.send_reply(Reply::Err(err));
    }

    fn schedule_rebind(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        let task = Schedulable::Rebind(eid, spec);
        let delay = self.config.retry_ivl;
        let _ = ctx.schedule(task, delay); 
        // TODO maybe we should keep track of the scheduled reconnection
        // In case the facade wants to close the ep somewhere between the error and the timeout
    }

    pub fn rebind(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        let pids = self.get_protocol_ids();
        let tmpl = EndpointTmpl {
            pids: pids,
            spec: spec
        };

        match ctx.rebind(self.id, eid, &tmpl) {
            Ok(_)  => self.on_rebind_success(ctx, eid, tmpl.spec),
            Err(_) => self.on_rebind_error(ctx, eid, tmpl.spec)
        };
    }

    fn on_rebind_success(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        let acceptor = Acceptor::from_spec(eid, spec);

        self.insert_acceptor(ctx, eid, acceptor)
    }

    fn on_rebind_error(&mut self, ctx: &mut Context, eid: EndpointId, spec: EndpointSpec) {
        self.schedule_rebind(ctx, eid, spec);
    }

/*****************************************************************************/
/*                                                                           */
/* pipe                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn on_pipe_opened(&mut self, ctx: &mut Context, eid: EndpointId) {
        if let Some(pipe) = self.pipes.remove(&eid) {
            self.protocol.add_pipe(ctx, eid, pipe);
        }
    }

    pub fn on_pipe_accepted(&mut self, ctx: &mut Context, aid: EndpointId, eid: EndpointId) {
        let pipe = self.accept_pipe(aid, eid);

        self.insert_pipe(ctx, eid, pipe);
    }

    pub fn close_pipe(&mut self, ctx: &mut Context, eid: EndpointId) {
        let _ = self.remove_pipe(ctx, eid);
    }

    pub fn on_pipe_error(&mut self, ctx: &mut Context, eid: EndpointId, _: io::Error) {
        if let Some(spec) = self.remove_pipe(ctx, eid) {
            self.schedule_reconnect(ctx, eid, spec);
        }
    }

    fn insert_pipe(&mut self, ctx: &mut Context, eid: EndpointId, pipe: Pipe) {
        pipe.open(ctx);

        self.pipes.insert(eid, pipe);
    }

    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<EndpointSpec> {
        if let Some(pipe) = self.pipes.remove(&eid) {
            return pipe.close(ctx)
        }
        if let Some(pipe) = self.protocol.remove_pipe(ctx, eid) {
            return pipe.close(ctx)
        }
        None
    }

    fn connect_pipe(&self, eid: EndpointId, spec: EndpointSpec) -> Pipe {
        Pipe::from_spec(eid, spec)
    }

    fn accept_pipe(&self, aid: EndpointId, eid: EndpointId) -> Pipe {
        let (send_prio, recv_prio) = if let Some(acceptor) = self.acceptors.get(&aid) {
            (acceptor.get_send_priority(), acceptor.get_recv_priority())
        } else {
            (self.config.send_priority, self.config.recv_priority)
        };
        let desc = EndpointDesc {
            send_priority: send_prio,
            recv_priority: recv_prio,
            tcp_no_delay: self.config.tcp_no_delay,
            recv_max_size: self.config.recv_max_size
        };

        Pipe::new_accepted(eid, desc)
    }

/*****************************************************************************/
/*                                                                           */
/* acceptor                                                                  */
/*                                                                           */
/*****************************************************************************/

    pub fn on_acceptor_error(&mut self, ctx: &mut Context, eid: EndpointId, _: io::Error) {
        if let Some(spec) = self.remove_acceptor(ctx, eid) {
            self.schedule_rebind(ctx, eid, spec);
        }
    }

    pub fn close_acceptor(&mut self, ctx: &mut Context, eid: EndpointId) {
        let _ = self.remove_acceptor(ctx, eid);
    }

    fn insert_acceptor(&mut self, ctx: &mut Context, eid: EndpointId, acceptor: Acceptor) {
        acceptor.open(ctx);

        self.acceptors.insert(eid, acceptor);
    }

    fn remove_acceptor(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<EndpointSpec> {
        self.acceptors.remove(&eid).map_or(None, |acceptor| acceptor.close(ctx))
    }

    fn connect_acceptor(&self, eid: EndpointId, spec: EndpointSpec) -> Acceptor {
        Acceptor::from_spec(eid, spec)
    }

/*****************************************************************************/
/*                                                                           */
/* send                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn send(&mut self, ctx: &mut Context, msg: Message) {
        #[cfg(debug_assertions)] debug!("[{:?}] send", ctx);
        if let Some(delay) = self.get_send_timeout() {
            let task = Schedulable::SendTimeout;

            match ctx.schedule(task, delay) {
                Ok(timeout) => self.protocol.send(ctx, msg, Some(timeout)),
                Err(e) => self.send_reply(Reply::Err(e))
            }
        } else {
            self.protocol.send(ctx, msg, None);
        }
    }

    pub fn try_send(&mut self, ctx: &mut Context, msg: Message) {
        #[cfg(debug_assertions)] debug!("[{:?}] try_send", ctx);
        if self.protocol.is_send_ready() {
            self.protocol.send(ctx, msg, None);
        } else {
            let err = would_block_io_error("socket is not send ready");

            self.send_reply(Reply::Err(err));
        }
    }

    pub fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId) {
        #[cfg(debug_assertions)] debug!("[{:?}] send ack from ep {:?}", ctx, eid);
        self.protocol.on_send_ack(ctx, eid);
    }

    pub fn on_send_timeout(&mut self, ctx: &mut Context) {
        #[cfg(debug_assertions)] debug!("[{:?}] send timeout", ctx);
        self.protocol.on_send_timeout(ctx);
    }

    fn get_send_timeout(&self) -> Option<Duration> {
        self.config.send_timeout
    }

    pub fn on_send_ready(&mut self, ctx: &mut Context, eid: EndpointId, ready: bool) {
        #[cfg(debug_assertions)] debug!("[{:?}] ep {:?} send ready: {} ", ctx, eid, ready);
        if ready {
            self.protocol.on_send_ready(ctx, eid)
        } else {
            self.protocol.on_send_not_ready(ctx, eid)
        }
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn recv(&mut self, ctx: &mut Context) {
        #[cfg(debug_assertions)] debug!("[{:?}] recv", ctx);
        if let Some(delay) = self.get_recv_timeout() {
            let task = Schedulable::RecvTimeout;

            match ctx.schedule(task, delay) {
                Ok(timeout) => self.protocol.recv(ctx, Some(timeout)),
                Err(e) => self.send_reply(Reply::Err(e))
            }
        } else {
            self.protocol.recv(ctx, None);
        }
    }

    pub fn try_recv(&mut self, ctx: &mut Context) {
        #[cfg(debug_assertions)] debug!("[{:?}] try_recv", ctx);
        if self.protocol.is_recv_ready() {
            self.protocol.recv(ctx, None);
        } else {
            let err = would_block_io_error("socket is not recv ready");
            
            self.send_reply(Reply::Err(err));
        }
    }

    pub fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, msg: Message) {
        #[cfg(debug_assertions)] debug!("[{:?}] recv ack from ep {:?}", ctx, eid);
        self.protocol.on_recv_ack(ctx, eid, msg);
    }

    pub fn on_recv_timeout(&mut self, ctx: &mut Context) {
        #[cfg(debug_assertions)] debug!("[{:?}] recv timeout", ctx);
        self.protocol.on_recv_timeout(ctx);
    }

    fn get_recv_timeout(&self) -> Option<Duration> {
        self.config.recv_timeout
    }

    pub fn on_recv_ready(&mut self, ctx: &mut Context, eid: EndpointId, ready: bool) {
        #[cfg(debug_assertions)] debug!("[{:?}] ep {:?} recv ready: {}", ctx, eid, ready);
        if ready {
            self.protocol.on_recv_ready(ctx, eid)
        } else {
            self.protocol.on_recv_not_ready(ctx, eid)
        }
    }

/*****************************************************************************/
/*                                                                           */
/* options                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn set_option(&mut self, _: &mut Context, opt: ConfigOption) {
        let res = if opt.is_generic() {
            self.config.set(opt)
        } else {
            self.protocol.set_option(opt)
        };
        let reply = match res {
            Ok(()) => Reply::SetOption,
            Err(e) => Reply::Err(e)
        };

        self.send_reply(reply);
    }

    pub fn on_timer_tick(&mut self, ctx: &mut Context, task: Schedulable) {
        self.protocol.on_timer_tick(ctx, task)
    }

    pub fn on_device_plugged(&mut self, ctx: &mut Context) {
        self.protocol.on_device_plugged(ctx)
    }

    pub fn close(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
        for (_, acceptor) in self.acceptors.drain() {
            acceptor.close(ctx);
        }

        self.protocol.close(ctx);

        ctx.raise(Event::Closed);
    }
}

/*****************************************************************************/
/*                                                                           */
/* tests                                                                     */
/*                                                                           */
/*****************************************************************************/

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::io;
    use std::time::Duration;

    use super::*;
    use core::network;
    use core::context::*;
    use core::{SocketId, EndpointId, Message, EndpointTmpl, Scheduled};
    use core::endpoint::Pipe;

    struct TestProto;

    impl Protocol for TestProto {
        fn id(&self) -> u16 {0}
        fn peer_id(&self) -> u16 {0}
        fn add_pipe(&mut self, _: &mut Context, _: EndpointId, _: Pipe) {}
        fn remove_pipe(&mut self, _: &mut Context, _: EndpointId) -> Option<Pipe> {None}
        fn send(&mut self, _: &mut Context, _: Message, _: Option<Scheduled>) {}
        fn on_send_ack(&mut self, _: &mut Context, _: EndpointId) {}
        fn on_send_timeout(&mut self, _: &mut Context) {}
        fn on_send_ready(&mut self, _: &mut Context, _: EndpointId) {}
        fn recv(&mut self, _: &mut Context, _: Option<Scheduled>) {}
        fn on_recv_ack(&mut self, _: &mut Context, _: EndpointId, _: Message) {}
        fn on_recv_timeout(&mut self, _: &mut Context) {}
        fn on_recv_ready(&mut self, _: &mut Context, _: EndpointId) {}
        fn is_send_ready(&self) -> bool { false }
        fn is_recv_ready(&self) -> bool { false }
        fn close(&mut self, _: &mut Context) {}
    }

    struct FailingNetwork;

    impl network::Network for FailingNetwork {
        fn connect(&mut self, _: SocketId, _: &EndpointTmpl) -> io::Result<EndpointId> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn reconnect(&mut self, _: SocketId, _: EndpointId, _: &EndpointTmpl) -> io::Result<()> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn bind(&mut self, _: SocketId, _: &EndpointTmpl) -> io::Result<EndpointId> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn rebind(&mut self, _: SocketId, _: EndpointId, _: &EndpointTmpl) -> io::Result<()> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn open(&mut self, _: EndpointId, _: bool) {
        }
        fn close(&mut self, _: EndpointId, _: bool) {
        }
        fn send(&mut self, _: EndpointId, _: Rc<Message>) {
        }
        fn recv(&mut self, _: EndpointId) {
        }
    }

    impl Scheduler for FailingNetwork {
        fn schedule(&mut self, _: Schedulable, _: Duration) -> io::Result<Scheduled> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn cancel(&mut self, _: Scheduled){
        }
    }

    impl fmt::Debug for FailingNetwork {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "FailingNetwork")
        }
    }

    impl Context for FailingNetwork {
        fn raise(&mut self, _: Event) {
        }
    }

    #[test]
    fn when_connect_fails() {
        let id = SocketId::from(1);
        let (tx, rx) = mpsc::channel();
        let proto = Box::new(TestProto) as Box<Protocol>;
        let mut network = FailingNetwork;
        let mut socket = Socket::new(id, tx, proto);

        socket.connect(&mut network, String::from("test://fake"));

        let reply = rx.recv().expect("Socket should have sent a reply to the connect request");

        match reply {
            Reply::Err(_) => {},
            _ => {
                assert!(false, "Socket should have replied an error to the connect request");
            },
        }
    }

    struct WorkingNetwork(EndpointId);

    impl network::Network for WorkingNetwork {
        fn connect(&mut self, _: SocketId, _: &EndpointTmpl) -> io::Result<EndpointId> {
            Ok(self.0)
        }
        fn reconnect(&mut self, _: SocketId, _: EndpointId, _: &EndpointTmpl) -> io::Result<()> {
            Ok(())
        }
        fn bind(&mut self, _: SocketId, _: &EndpointTmpl) -> io::Result<EndpointId> {
            Ok(self.0)
        }
        fn rebind(&mut self, _: SocketId, _: EndpointId, _: &EndpointTmpl) -> io::Result<()> {
            Ok(())
        }
        fn open(&mut self, _: EndpointId, _: bool) {}
        fn close(&mut self, _: EndpointId, _: bool) {}
        fn send(&mut self, _: EndpointId, _: Rc<Message>) {}
        fn recv(&mut self, _: EndpointId) {}
    }

    impl Scheduler for WorkingNetwork {
        fn schedule(&mut self, _: Schedulable, _: Duration) -> io::Result<Scheduled> {
            Ok(Scheduled::from(0))
        }
        fn cancel(&mut self, _: Scheduled){
        }
    }

    impl fmt::Debug for WorkingNetwork {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "WorkingNetwork")
        }
    }

    impl Context for WorkingNetwork {
        fn raise(&mut self, _: Event) {

        }
    }

    #[test]
    fn when_connect_succeeds() {
        let id = SocketId::from(1);
        let (tx, rx) = mpsc::channel();
        let proto = Box::new(TestProto) as Box<Protocol>;
        let mut network = WorkingNetwork(EndpointId::from(1));
        let mut socket = Socket::new(id, tx, proto);

        socket.connect(&mut network, String::from("test://fake"));

        let reply = rx.recv().expect("Socket should have sent a reply to the connect request");

        match reply {
            Reply::Connect(eid) => {
                assert_eq!(EndpointId::from(1), eid);
            },
            _ => {
                assert!(false, "Socket should have replied an ack to the connect request");
            },
        }
    }
}
