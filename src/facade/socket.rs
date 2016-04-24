// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::sync::mpsc::Receiver;
use std::time;

use mio;
use mio::Sender;

use global::*;
use event_loop_msg::*;
use Message;
use facade::endpoint::Endpoint;

/// Socket is the main access that applications use to access the SP system.  
/// It is an abstraction of an application's "connection" to a messaging topology.  
/// Applications can have more than one Socket open at a time.
pub struct Socket {
    id: SocketId,
    socket_type: SocketType,
    cmd_sender: Sender<EventLoopSignal>,
    evt_receiver: Receiver<SocketNotify> /* Could use https://github.com/polyfractal/bounded-spsc-queue ?
                                          * Maybe once a smart waiting strategy is available (like spin, then sleep 0, then sleep 1, then mutex ?)
                                          * or something that would help for poll */
}

impl Socket {
    #[doc(hidden)]
    pub fn new(id: SocketId, socket_type: SocketType, cmd_tx: Sender<EventLoopSignal>, evt_rx: Receiver<SocketNotify>) -> Socket {
        Socket {
            id: id,
            socket_type: socket_type,
            cmd_sender: cmd_tx,
            evt_receiver: evt_rx
        }
    }

    #[doc(hidden)]
    pub fn get_id(&self) -> SocketId {
        self.id
    }

    #[doc(hidden)]
    pub fn get_socket_type(&self) -> SocketType {
        self.socket_type
    }

    fn send_cmd(&self, cmd: SocketCmdSignal) -> Result<(), io::Error> {
        let cmd_sig = CmdSignal::Socket(self.id, cmd);
        let loop_sig = EventLoopSignal::Cmd(cmd_sig);

        self.cmd_sender.send(loop_sig).map_err(convert_notify_err)
    }

    /// Adds a remote endpoint to the socket.
    /// The library would then try to connect to the specified remote endpoint.
    /// The addr argument consists of two parts as follows: `transport://address`.
    /// The transport specifies the underlying transport protocol to use.
    /// The meaning of the address part is specific to the underlying transport protocol.
    /// Note that bind and connect may be called multiple times on the same socket,
    /// thus allowing the socket to communicate with multiple heterogeneous endpoints.
    /// On success, returns an [Endpoint](struct.Endpoint.html) that can be later used to remove the endpoint from the socket.
    pub fn connect(&mut self, addr: &str) -> Result<Endpoint, io::Error> {
        let cmd = SocketCmdSignal::Connect(addr.to_owned());

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::Connected(t)) => Ok(self.new_endpoint(t)),
            Ok(SocketNotify::NotConnected(e)) => Err(e),
            Ok(_) => Err(other_io_error("unexpected evt")),
            Err(_) => Err(other_io_error("evt channel closed")),
        }
    }

    /// Adds a local endpoint to the socket. The endpoint can be then used by other applications to connect to.
    /// The addr argument consists of two parts as follows: `transport://address`.
    /// The transport specifies the underlying transport protocol to use.
    /// The meaning of the address part is specific to the underlying transport protocol.
    /// Note that bind and connect may be called multiple times on the same socket,
    /// thus allowing the socket to communicate with multiple heterogeneous endpoints.
    /// On success, returns an [Endpoint](struct.Endpoint.html) that can be later used to remove the endpoint from the socket.
    pub fn bind(&mut self, addr: &str) -> Result<Endpoint, io::Error> {
        let cmd = SocketCmdSignal::Bind(addr.to_owned());

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::Bound(t)) => Ok(self.new_endpoint(t)),
            Ok(SocketNotify::NotBound(e)) => Err(e),
            Ok(_) => Err(other_io_error("unexpected evt")),
            Err(_) => Err(other_io_error("evt channel closed")),
        }
    }

    fn new_endpoint(&self, tok: mio::Token) -> Endpoint {
        Endpoint::new(self.id, tok, self.cmd_sender.clone())
    }

    /// Sends a message.
    pub fn send(&mut self, buffer: Vec<u8>) -> Result<(), io::Error> {
        self.send_msg(Message::with_body(buffer))
    }

    /// Sends a message.
    pub fn send_msg(&mut self, msg: Message) -> Result<(), io::Error> {
        let cmd = SocketCmdSignal::SendMsg(msg);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::MsgSent) => Ok(()),
            Ok(SocketNotify::MsgNotSent(e)) => Err(e),
            Ok(_) => Err(other_io_error("unexpected evt")),
            Err(_) => Err(other_io_error("evt channel closed")),
        }
    }

    /// Receives a message.
    pub fn recv(&mut self) -> Result<Vec<u8>, io::Error> {
        self.recv_msg().map(|msg| msg.into_buffer())
    }

    /// Receives a message.
    pub fn recv_msg(&mut self) -> Result<Message, io::Error> {
        let cmd = SocketCmdSignal::RecvMsg;

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::MsgRecv(msg)) => Ok(msg),
            Ok(SocketNotify::MsgNotRecv(e)) => Err(e),
            Ok(_) => Err(other_io_error("unexpected evt")),
            Err(_) => Err(other_io_error("evt channel closed")),
        }
    }

    /// Sets a socket option.
    /// See [SocketOption](enum.SocketOption.html) to get the list of options.
    pub fn set_option(&mut self, option: SocketOption) -> io::Result<()> {
        let cmd = SocketCmdSignal::SetOption(option);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::OptionSet) => Ok(()),
            Ok(SocketNotify::OptionNotSet(e)) => Err(e),
            Ok(_) => Err(other_io_error("unexpected evt")),
            Err(_) => Err(other_io_error("evt channel closed")),
        }
    }

    /// Sets the timeout for send operation on the socket.  
    /// If message cannot be sent within the specified timeout, 
    /// an error with the kind `TimedOut` is returned. 
    /// Zero value means infinite timeout. Default value is zero.
    pub fn set_send_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::SendTimeout(timeout))
    }

    /// Sets the timeout for recv operation on the socket.  
    /// If message cannot be received within the specified timeout, 
    /// an error with the kind `TimedOut` is returned. 
    /// Zero value means infinite timeout. Default value is zero.
    pub fn set_recv_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::RecvTimeout(timeout))
    }

    /// Sets outbound priority for endpoints subsequently added to the socket.  
    /// This option has no effect on socket types that send messages to all the peers.  
    /// However, if the socket type sends each message to a single peer (or a limited set of peers), 
    /// peers with high priority take precedence over peers with low priority. 
    /// Highest priority is 1, lowest priority is 16. Default value is 8.
    pub fn set_send_priority(&mut self, priority: u8) -> io::Result<()> {
        self.set_option(SocketOption::SendPriority(priority))
    }

    /// This option, when set to `true`, disables Nagle’s algorithm.
    /// It also disables delaying of TCP acknowledgments. 
    /// Using this option improves latency at the expense of throughput.
    /// Default value is `false`.
    pub fn set_tcp_nodelay(&mut self, value: bool) -> io::Result<()> {
        self.set_option(SocketOption::TcpNoDelay(value))
    }

    /// Sets inbound priority for endpoints subsequently added to the socket.  
    /// This option has no effect on socket types that are not able to receive messages.  
    /// When receiving a message, messages from peer with higher priority 
    /// are received before messages from peer with lower priority. 
    /// Highest priority is 1, lowest priority is 16. Default value is 8.
    pub fn set_recv_priority(&mut self, priority: u8) -> io::Result<()> {
        self.set_option(SocketOption::RecvPriority(priority))
    }

    pub fn set_reconnect_ivl(&mut self, ivl: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::ReconnectInterval(ivl))
    }

    pub fn set_reconnect_ivl_max(&mut self, ivl: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::ReconnectIntervalMax(ivl))
    }

    #[doc(hidden)]
    pub fn matches(&self, other: &Socket) -> bool {
        self.socket_type.matches(other.socket_type)
    }

    #[doc(hidden)]
    pub fn forward_msg(&mut self, other: &mut Socket) -> io::Result<()> {
        self.recv_msg().and_then(|msg| other.send_msg(msg))
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let cmd = SessionCmdSignal::DestroySocket(self.id);
        let cmd_sig = CmdSignal::Session(cmd);
        let loop_sig = EventLoopSignal::Cmd(cmd_sig);

        let _ = self.cmd_sender.send(loop_sig);
    }
}
