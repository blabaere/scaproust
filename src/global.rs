// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::fmt;
use std::rc::Rc;
use std::cell::Cell;
use std::io::{Error, ErrorKind};
use std::time;

use mio::NotifyError;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SocketType {

    /// **One-to-one protocol**
    /// Pair protocol is the simplest and least scalable scalability protocol. 
    /// It allows scaling by breaking the application in exactly two pieces. 
    /// For example, if a monolithic application handles both accounting and agenda of HR department, 
    /// it can be split into two applications (accounting vs. HR) that are run on two separate servers. 
    /// These applications can then communicate via PAIR sockets. 
    /// The downside of this protocol is that its scaling properties are very limited. 
    /// Splitting the application into two pieces allows to scale the two servers. 
    /// To add the third server to the cluster, application has to be split once more, 
    /// say by separating HR functionality into hiring module and salary computation module. 
    /// Whenever possible, try to use one of the more scalable protocols instead.  
    ///  
    /// Socket for communication with exactly one peer.
    /// Each party can send messages at any time. 
    /// If the peer is not available or send buffer is full subsequent calls to [send](struct.Socket.html#method.send) 
    /// will block until it’s possible to send the message.
    Pair       = (    16),

    /// **Publish/subscribe protocol**
    /// Broadcasts messages to multiple destinations.
    /// Messages are sent from `Pub` sockets and will only be received 
    /// by `Sub` sockets that have subscribed to the matching topic. 
    /// Topic is an arbitrary sequence of bytes at the beginning of the message body. 
    /// The `Sub` socket will determine whether a message should be delivered 
    /// to the user by comparing the subscribed topics to the bytes initial bytes 
    /// in the incomming message, up to the size of the topic.  
    /// Subscribing via [set_option](struct.Socket.html#method.set_option) and [SocketOption::Subscribe](enum.SocketOption.html#variant.Subscribe)
    /// Will match any message with intial 5 bytes being "Hello", for example, message "Hello, World!" will match.
    /// Topic with zero length matches any message.
    /// If the socket is subscribed to multiple topics, 
    /// message matching any of them will be delivered to the user.
    /// Since the filtering is performed on the Subscriber side, 
    /// all the messages from Publisher will be sent over the transport layer.
    /// The entire message, including the topic, is delivered to the user.  
    ///   
    /// This socket is used to distribute messages to multiple destinations. Receive operation is not defined.
    Pub        = (2 * 16),

    /// Receives messages from the publisher. 
    /// Only messages that the socket is subscribed to are received. 
    /// When the socket is created there are no subscriptions 
    /// and thus no messages will be received. 
    /// Send operation is not defined on this socket.
    Sub        = (2 * 16) + 1,

    /// 
    Req        = (3 * 16),
    Rep        = (3 * 16) + 1,
    Push       = (5 * 16),
    Pull       = (5 * 16) + 1,
    Surveyor   = (6 * 16) + 2,
    Respondent = (6 * 16) + 3,
    Bus        = (7 * 16)
}

impl SocketType {
    pub fn id(&self) -> u16 {
        *self as u16
    }

    pub fn peer(&self) -> SocketType {
        match *self {
            SocketType::Pair       => SocketType::Pair,
            SocketType::Pub        => SocketType::Sub,
            SocketType::Sub        => SocketType::Pub,
            SocketType::Req        => SocketType::Rep,
            SocketType::Rep        => SocketType::Req,
            SocketType::Push       => SocketType::Pull,
            SocketType::Pull       => SocketType::Push,
            SocketType::Surveyor   => SocketType::Respondent,
            SocketType::Respondent => SocketType::Surveyor,
            SocketType::Bus        => SocketType::Bus,
        }
    }

    pub fn matches(&self, other: SocketType) -> bool {
        self.peer() == other && other.peer() == *self
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct SocketId(pub usize);

impl fmt::Debug for SocketId {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ProbeId(pub usize);

impl fmt::Debug for ProbeId {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone)]
pub struct IdSequence {
    value: Rc<Cell<usize>>
}

impl IdSequence {
    pub fn new() -> IdSequence {
        IdSequence { value: Rc::new(Cell::new(0)) }
    }

    pub fn next(&self) -> usize {
        let id = self.value.get();

        self.value.set(id + 1);
        id
    }
}

impl Default for IdSequence {
    fn default() -> Self {
        IdSequence::new()
    }
}

pub fn other_io_error(msg: &'static str) -> Error {
    Error::new(ErrorKind::Other, msg)
}

pub fn invalid_data_io_error(msg: &'static str) -> Error {
    Error::new(ErrorKind::InvalidData, msg)
}

pub fn would_block_io_error(msg: &'static str) -> Error {
    Error::new(ErrorKind::WouldBlock, msg)
}

pub fn invalid_input_io_error(msg: &'static str) -> Error {
    Error::new(ErrorKind::InvalidInput, msg)
}

pub fn convert_notify_err<T>(err: NotifyError<T>) -> Error {
    match err {
        NotifyError::Io(e) => e,
        NotifyError::Closed(_) => other_io_error("cmd channel closed"),
        NotifyError::Full(_) => Error::new(ErrorKind::WouldBlock, "cmd channel full"),
    }
}

pub trait ToMillis {
    fn to_millis(&self) -> u64;
}

impl ToMillis for time::Duration {
    fn to_millis(&self) -> u64 {
        let millis_from_secs = self.as_secs() * 1_000;
        let millis_from_nanos = self.subsec_nanos() as f64 / 1_000_000f64;

        millis_from_secs + millis_from_nanos as u64
    }
}

#[cfg(test)]
mod tests {
    use super::IdSequence;

    #[test]
    fn id_sequence_can_be_cloned() {
        let seq = IdSequence::new();
        let other = seq.clone();

        assert_eq!(0, other.next());
        assert_eq!(1, seq.next());
        assert_eq!(2, seq.next());
        assert_eq!(3, other.next());
    }
}
