// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::collections::HashMap;

use std::sync::mpsc;

use mio;

use global::*;
use event_loop_msg::*;

use socket::Socket;
use protocol;

use EventLoop;
use Message;

pub struct Session {
    event_sender: mpsc::Sender<SessionNotify>,
    sockets: HashMap<SocketId, Socket>,
    socket_ids: HashMap<mio::Token, SocketId>,
    id_seq: IdSequence
}

impl Session {

    pub fn new(event_tx: mpsc::Sender<SessionNotify>) -> Session {
        Session {
            event_sender: event_tx,
            sockets: HashMap::new(),
            socket_ids: HashMap::new(),
            id_seq: IdSequence::new()
        }
    }

    fn handle_cmd(&mut self, event_loop: &mut EventLoop, cmd: CmdSignal) {
        match cmd {
            CmdSignal::Session(c)    => self.handle_session_cmd(event_loop, c),
            CmdSignal::Socket(id, c) => self.handle_socket_cmd(event_loop, id, c)
        }
    }

    fn handle_session_cmd(&mut self, event_loop: &mut EventLoop, cmd: SessionCmdSignal) {
        match cmd {
            SessionCmdSignal::CreateSocket(t) => self.create_socket(event_loop, t),
            SessionCmdSignal::Shutdown        => event_loop.shutdown(),
        }
    }

    fn handle_socket_cmd(&mut self, event_loop: &mut EventLoop, id: SocketId, cmd: SocketCmdSignal) {
        if let Some(socket) = self.sockets.get_mut(&id) {
            socket.handle_cmd(event_loop, cmd);
        }
    }

    fn handle_evt(&mut self, event_loop: &mut EventLoop, evt: EvtSignal) {
        match evt {
            EvtSignal::Socket(id, e) => self.handle_socket_evt(event_loop, id, e),
            EvtSignal::Pipe(tok, e) => self.handle_pipe_evt(event_loop, tok, e)
        }
    }

    fn handle_socket_evt(&mut self, event_loop: &mut EventLoop, id: SocketId, evt: SocketEvtSignal) {
        match evt {
            SocketEvtSignal::Connected(tok) => {
                self.socket_ids.insert(tok, id);

                if let Some(socket) = self.sockets.get_mut(&id) {
                    socket.handle_evt(event_loop, SocketEvtSignal::Connected(tok));
                }
           }
        }
    }

    fn handle_pipe_evt(&mut self, event_loop: &mut EventLoop, tok: mio::Token, evt: PipeEvtSignal) {
        match evt {
            PipeEvtSignal::MsgRcv(msg) => {

                if let Some(socket_id) = self.socket_ids.get(&tok) {
                    if let Some(socket) = self.sockets.get_mut(&socket_id) {
                        socket.on_msg_recv(event_loop, msg);
                    }
                }

            }
        }
    }

    fn send_evt(&self, evt: SessionNotify) {
        let send_res = self.event_sender.send(evt);

        if send_res.is_err() {
            error!("failed to notify event to session: '{:?}'", send_res.err());
        } 
    }

    fn create_socket(&mut self, event_loop: &mut EventLoop, socket_type: SocketType) {
        let id = SocketId(self.id_seq.next());
        let (tx, rx) = mpsc::channel();
        let evt_tx = Rc::new(tx);
        let sig_tx = event_loop.channel();
        let protocol = protocol::create_protocol(id, socket_type, evt_tx.clone());
        let socket = Socket::new(id, protocol, evt_tx.clone(), sig_tx, self.id_seq.clone());

        self.sockets.insert(id, socket);

        self.send_evt(SessionNotify::SocketCreated(id, rx));
    }

    fn do_on_socket<F>(&mut self, socket_id: SocketId, action: &mut F) where F : FnMut(&mut Socket) {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            action(socket);
        }
    }

    fn reconnect(&mut self, event_loop: &mut EventLoop, token: mio::Token, addr: String) {
        if let Some(socket_id) = self.socket_ids.get_mut(&token) {
            if let Some(socket) = self.sockets.get_mut(&socket_id) {
                socket.reconnect(addr, event_loop, token);
            }
        }
    }

    fn bind(&mut self, event_loop: &mut EventLoop, id: SocketId, addr: String) {
        // let the socket allocate the token and signal it
        let token = mio::Token(self.id_seq.next());

        self.socket_ids.insert(token, id);

        if let Some(socket) = self.sockets.get_mut(&id) {
            socket.bind(addr, event_loop, token);
        }
    }

    fn rebind(&mut self, event_loop: &mut EventLoop, token: mio::Token, addr: String) {
        if let Some(socket_id) = self.socket_ids.get_mut(&token) {
            if let Some(socket) = self.sockets.get_mut(&socket_id) {
                socket.rebind(addr, event_loop, token);
            }
        }
    }

    fn send(&mut self, event_loop: &mut EventLoop, id: SocketId, msg: Message) {
        if let Some(socket) = self.sockets.get_mut(&id) {
            socket.send(event_loop, msg);
        }
    }

    fn on_send_timeout(&mut self, event_loop: &mut EventLoop, socket_id: SocketId) {
        self.do_on_socket(socket_id, &mut |socket| socket.on_send_timeout(event_loop));
    }

    fn recv(&mut self, event_loop: &mut EventLoop, id: SocketId) {
        if let Some(socket) = self.sockets.get_mut(&id) {
            socket.recv(event_loop);
        }
    }

    fn on_recv_timeout(&mut self, event_loop: &mut EventLoop, id: SocketId) {
        self.do_on_socket(id, &mut |socket| socket.on_recv_timeout(event_loop));
    }

    fn on_survey_timeout(&mut self, event_loop: &mut EventLoop, id: SocketId) {
        self.do_on_socket(id, &mut |socket| socket.on_survey_timeout(event_loop));
    }

    fn on_request_timeout(&mut self, event_loop: &mut EventLoop, id: SocketId) {
        self.do_on_socket(id, &mut |socket| socket.on_request_timeout(event_loop));
    }

    fn link(&mut self, mut tokens: Vec<mio::Token>, id: SocketId) {
        for token in tokens.drain(..) {
            self.socket_ids.insert(token, id);
        }
    }

    fn set_option(&mut self, event_loop: &mut EventLoop, id: SocketId, opt: SocketOption) {
        if let Some(socket) = self.sockets.get_mut(&id) {
            socket.set_option(event_loop, opt);
        }
    }
}

impl mio::Handler for Session {
    type Timeout = EventLoopTimeout;
    type Message = EventLoopSignal;

    fn notify(&mut self, event_loop: &mut EventLoop, signal: Self::Message) {
        debug!("session received a signal");

        match signal {
            EventLoopSignal::Cmd(cmd) => self.handle_cmd(event_loop, cmd),
            EventLoopSignal::Evt(evt) => self.handle_evt(event_loop, evt)
        }
    }

    fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) {
        debug!("ready: '{:?}' '{:?}'", token, events);
        let mut target_socket_id = None;
        let mut added_tokens = None;

        if let Some(socket_id) = self.socket_ids.get(&token) {
            if let Some(socket) = self.sockets.get_mut(&socket_id) {
                target_socket_id = Some(socket_id.clone());
                added_tokens = socket.ready(event_loop, token, events);
            }
        }

        if let Some(socket_id) = target_socket_id {
            if let Some(tokens) = added_tokens {
                self.link(tokens, socket_id);
            }
        }
        
    }

    fn timeout(&mut self, event_loop: &mut EventLoop, timeout: Self::Timeout) {
        match timeout {
            EventLoopTimeout::Reconnect(token, addr)  => self.reconnect(event_loop, token, addr),
            EventLoopTimeout::Rebind(token, addr)     => self.rebind(event_loop, token, addr),
            EventLoopTimeout::CancelSend(socket_id)   => self.on_send_timeout(event_loop, socket_id),
            EventLoopTimeout::CancelRecv(socket_id)   => self.on_recv_timeout(event_loop, socket_id),
            EventLoopTimeout::CancelSurvey(socket_id) => self.on_survey_timeout(event_loop, socket_id),
            EventLoopTimeout::CancelResend(socket_id) => self.on_request_timeout(event_loop, socket_id)
        }
    }

    fn interrupted(&mut self, _: &mut EventLoop) {

    }
}
