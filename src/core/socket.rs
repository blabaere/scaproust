// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::io;
use std::boxed::FnBox;
use std::time::Duration;

use super::{SocketId, EndpointId, Message};
use super::endpoint::{Pipe, Acceptor};
use super::config::{Config, ConfigOption};
use super::context::{Context, Schedulable, Scheduled};

pub enum Request {
    Connect(String),
    Bind(String),
    Send(Message),
    Recv,
    SetOption(ConfigOption),
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
    pipes: HashMap<EndpointId, Pipe>,
    acceptors: HashMap<EndpointId, Acceptor>,
    config: Config
}

pub trait Protocol {
    fn id(&self) -> u16;
    fn peer_id(&self) -> u16;

    fn add_pipe(&mut self, ctx: &mut Context, eid: EndpointId, pipe: Pipe);
    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<Pipe>;

    fn send(&mut self, ctx: &mut Context, msg: Message, timeout: Option<Scheduled>);
    fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId);
    fn on_send_timeout(&mut self, ctx: &mut Context);
    
    fn recv(&mut self, ctx: &mut Context);
    fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, msg: Message);
}

pub type ProtocolCtor = Box<FnBox(Sender<Reply>) -> Box<Protocol> + Send>;

impl Socket {
    pub fn new(id: SocketId, reply_tx: Sender<Reply>, proto: Box<Protocol>) -> Socket {
        Socket {
            id: id,
            reply_sender: reply_tx,
            protocol: proto,
            pipes: HashMap::new(),
            acceptors: HashMap::new(),
            config: Config::new()
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

/*****************************************************************************/
/*                                                                           */
/* connect                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn connect(&mut self, ctx: &mut Context, url: String) {
        let pids = self.get_protocol_ids();

        match ctx.connect(self.id, &url, pids) {
            Ok(id) => self.on_connect_success(ctx, url, id),
            Err(e) => self.on_connect_error(e)
        };
    }

    fn on_connect_success(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
        self.insert_pipe(ctx, eid, Pipe::new_created(eid, url));
        self.send_reply(Reply::Connect(eid));
    }

    fn on_connect_error(&mut self, err: io::Error) {
        self.send_reply(Reply::Err(err));
    }

    fn schedule_reconnect(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
        let task = Schedulable::Reconnect(self.id, eid, url);
        let delay = self.config.retry_ivl;
        let _ = ctx.schedule(task, delay); 
        // TODO maybe we should keep track of the scheduled reconnection
        // In case the facade wants to close the pipe somewhere between the error and the timeout
    }

    pub fn reconnect(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
        let pids = self.get_protocol_ids();

        match ctx.reconnect(self.id, eid, &url, pids) {
            Ok(_)  => self.on_reconnect_success(ctx, url, eid),
            Err(_) => self.on_reconnect_error(ctx, url, eid)
        }
    }

    fn on_reconnect_success(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
        self.insert_pipe(ctx, eid, Pipe::new_created(eid, url));
    }

    fn on_reconnect_error(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
        self.schedule_reconnect(ctx, url, eid);
    }

/*****************************************************************************/
/*                                                                           */
/* bind                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn bind(&mut self, ctx: &mut Context, url: String) {
        let pids = self.get_protocol_ids();

        match ctx.bind(self.id, &url, pids) {
            Ok(id) => self.on_bind_success(ctx, url, id),
            Err(e) => self.on_bind_error(e)
        };
    }

    fn on_bind_success(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
        let acceptor = Acceptor::new(eid, url);

        acceptor.open(ctx);

        self.acceptors.insert(eid, acceptor);
        self.send_reply(Reply::Bind(eid));
    }

    fn on_bind_error(&mut self, err: io::Error) {
        self.send_reply(Reply::Err(err));
    }

    fn schedule_rebind(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
    }

    pub fn rebind(&mut self, ctx: &mut Context, url: String, eid: EndpointId) {
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

    pub fn on_pipe_accepted(&mut self, ctx: &mut Context, eid: EndpointId) {
        let pipe = Pipe::new_accepted(eid);

        pipe.open(ctx);

        self.pipes.insert(eid, pipe);
    }

    pub fn on_pipe_error(&mut self, ctx: &mut Context, eid: EndpointId, _: io::Error) {
        if let Some(url) = self.remove_pipe(ctx, eid) {
            self.schedule_reconnect(ctx, url, eid);
        }
    }

    fn insert_pipe(&mut self, ctx: &mut Context, eid: EndpointId, pipe: Pipe) {
        pipe.open(ctx);

        self.pipes.insert(eid, pipe);
    }

    fn remove_pipe(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<String> {
        if let Some(pipe) = self.pipes.remove(&eid) {
            return pipe.close(ctx)
        }
        if let Some(pipe) = self.protocol.remove_pipe(ctx, eid) {
            return pipe.close(ctx)
        }
        None
    }

/*****************************************************************************/
/*                                                                           */
/* acceptor                                                                  */
/*                                                                           */
/*****************************************************************************/

    pub fn on_acceptor_error(&mut self, ctx: &mut Context, eid: EndpointId, _: io::Error) {
        if let Some(url) = self.remove_acceptor(ctx, eid) {
            self.schedule_rebind(ctx, url, eid);
        }
    }

    fn remove_acceptor(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<String> {
        if let Some(acceptor) = self.acceptors.remove(&eid) {
            acceptor.close(ctx)
        } else {
            None
        }
    }

/*****************************************************************************/
/*                                                                           */
/* send                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn send(&mut self, ctx: &mut Context, msg: Message) {
        if let Some(delay) = self.get_send_timeout() {
            let task = Schedulable::SendTimeout(self.id);

            match ctx.schedule(task, delay) {
                Ok(timeout) => self.protocol.send(ctx, msg, Some(timeout)),
                Err(e) => self.send_reply(Reply::Err(e))
            }
        } else {
            self.protocol.send(ctx, msg, None);
        }
    }

    pub fn on_send_ack(&mut self, ctx: &mut Context, eid: EndpointId) {
        self.protocol.on_send_ack(ctx, eid);
    }

    fn get_send_timeout(&self) -> Option<Duration> {
        self.config.send_timeout.as_ref().map(|d| d.clone())
    }

    pub fn on_send_timeout(&mut self, ctx: &mut Context) {
        self.protocol.on_send_timeout(ctx);
    }

/*****************************************************************************/
/*                                                                           */
/* recv                                                                      */
/*                                                                           */
/*****************************************************************************/

    pub fn recv(&mut self, ctx: &mut Context) {
        self.protocol.recv(ctx);
    }

    pub fn on_recv_ack(&mut self, ctx: &mut Context, eid: EndpointId, msg: Message) {
        self.protocol.on_recv_ack(ctx, eid, msg);
    }

/*****************************************************************************/
/*                                                                           */
/* options                                                                   */
/*                                                                           */
/*****************************************************************************/

    pub fn set_opt(&mut self, _: &mut Context, opt: ConfigOption) {
        let reply = match self.config.set(opt) {
            Ok(()) => Reply::SetOption,
            Err(e) => Reply::Err(e)
        };

        self.send_reply(reply);
    }

}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::io;
    use std::time::Duration;

    use super::*;
    use core::network::Network;
    use core::context::*;
    use core::{SocketId, EndpointId, Message};
    use core::endpoint::Pipe;
    use io_error::*;

    struct TestProto;

    impl Protocol for TestProto {
        fn id(&self) -> u16 {0}
        fn peer_id(&self) -> u16 {0}
        fn add_pipe(&mut self, _: &mut Context, _: EndpointId, _: Pipe) {}
        fn remove_pipe(&mut self, _: &mut Context, _: EndpointId) -> Option<Pipe> {None}
        fn send(&mut self, _: &mut Context, _: Message, _: Option<Scheduled>) {}
        fn on_send_ack(&mut self, _: &mut Context, _: EndpointId) {}
        fn on_send_timeout(&mut self, _: &mut Context) {}
        fn recv(&mut self, _: &mut Context) {}
        fn on_recv_ack(&mut self, _: &mut Context, _: EndpointId, _: Message) {}
    }

    struct FailingNetwork;

    impl Network for FailingNetwork {
        fn connect(&mut self, _: SocketId, _: &str, _: (u16, u16)) -> io::Result<EndpointId> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn reconnect(&mut self, _: SocketId, _: EndpointId, _: &str, _: (u16, u16)) -> io::Result<()> {
            Err(other_io_error("FailingNetwork can only fail"))
        }
        fn bind(&mut self, _: SocketId, _: &str, _: (u16, u16)) -> io::Result<EndpointId> {
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

    impl Network for WorkingNetwork {
        fn connect(&mut self, _: SocketId, _: &str, _: (u16, u16)) -> io::Result<EndpointId> {
            Ok(self.0)
        }
        fn reconnect(&mut self, _: SocketId, _: EndpointId, _: &str, _: (u16, u16)) -> io::Result<()> {
            Ok(())
        }
        fn bind(&mut self, _: SocketId, _: &str, _: (u16, u16)) -> io::Result<EndpointId> {
            Ok(self.0)
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
