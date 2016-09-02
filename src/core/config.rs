// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io::Result;
use std::time::Duration;

use io_error::*;

pub struct Config {
    pub send_timeout: Option<Duration>,
    pub send_priority: u8,
    pub recv_timeout: Option<Duration>,
    pub recv_priority: u8,
    pub retry_ivl: Duration,
    pub retry_ivl_max: Option<Duration>
}

pub enum ConfigOption {
    /// Specifies how long the socket should try to send pending outbound messages 
    /// after `drop` have been called. Default value is 1 second.
    Linger(Duration),

    /// See [Socket::set_send_timeout](struct.Socket.html#method.set_send_timeout).
    SendTimeout(Option<Duration>),

    /// See [Socket::set_send_priority](struct.Socket.html#method.set_send_priority).
    SendPriority(u8),

    /// See [Socket::set_recv_timeout](struct.Socket.html#method.set_recv_timeout).
    RecvTimeout(Option<Duration>),

    /// See [Socket::set_recv_priority](struct.Socket.html#method.set_recv_priority).
    RecvPriority(u8),

    /// Maximum message size that can be received, in bytes. 
    /// Zero value means that the received size is limited only by available addressable memory. 
    /// Default is 1024kB.
    RecvMaxSize(u64),

    /// For connection-based transports such as TCP, this option specifies how long to wait, 
    /// when connection is broken before trying to re-establish it. 
    /// Note that actual reconnect interval may be randomised to some extent 
    /// to prevent severe reconnection storms. Default value is 0.1 second.
    RetryIvl(Duration),

    /// This option is to be used only in addition to ReconnectInterval option.
    /// It specifies maximum reconnection interval. On each reconnect attempt,
    /// the previous interval is doubled until ReconnectIntervalMax is reached.
    /// Value of `None` means that no exponential backoff is performed and reconnect interval is based only on ReconnectInterval.
    /// If RetryIvlMax is less than RetryIvl, it is ignored. 
    /// Default value is `None`.
    RetryIvlMax(Option<Duration>),

    /// See [Socket::set_tcp_nodelay](struct.Socket.html#method.set_tcp_nodelay).
    TcpNoDelay(bool),

    /// Defined on `Sub` socket. Subscribes for a particular topic.
    /// A single `Sub` socket can handle multiple subscriptions.
    Subscribe(String),

    /// Defined on Sub` socket. Unsubscribes from a particular topic.
    Unsubscribe(String),

    /// This option is defined on the Req socket.
    /// If a reply is not received in the specified amount of time, 
    /// the request will be automatically resent. 
    /// Default value is 1 minute.
    ReqResendIvl(Duration),

    /// Specifies how long to wait for responses to the survey.
    /// Once the deadline expires, receive function will return a TimedOut error 
    /// and all subsequent responses to the survey will be silently dropped.
    /// Default value is 1 second.
    SurveyDeadline(Duration)
}

impl Default for Config {
    fn default() -> Config {
        Config {
            send_timeout: None,
            send_priority: 8,
            recv_timeout: None,
            recv_priority: 8,
            retry_ivl: Duration::from_millis(100),
            retry_ivl_max: None
        }
    }
}

impl Config {
    pub fn set(&mut self, cfg_opt: ConfigOption) -> Result<()> {
        match cfg_opt {
            ConfigOption::SendTimeout(timeout) => self.send_timeout = timeout,
            ConfigOption::SendPriority(priority) => self.send_priority = priority,
            ConfigOption::RecvTimeout(timeout) => self.recv_timeout = timeout,
            ConfigOption::RecvPriority(priority) => self.recv_priority = priority,
            ConfigOption::RetryIvl(ivl) => self.retry_ivl = ivl,
            ConfigOption::RetryIvlMax(ivl) => self.retry_ivl_max = ivl,
            _ => return Err(invalid_input_io_error("option not supported"))
        }
        Ok(())
    }
}

impl ConfigOption {
    #[doc(hidden)]
    pub fn is_generic(&self) -> bool {
        match *self {
            ConfigOption::Linger(_)       |
            ConfigOption::SendTimeout(_)  |
            ConfigOption::SendPriority(_) |
            ConfigOption::RecvTimeout(_)  |
            ConfigOption::RecvPriority(_) |
            ConfigOption::RetryIvl(_)     |
            ConfigOption::RetryIvlMax(_)  |
            ConfigOption::TcpNoDelay(_)   => true,
            _ => false
        }
    }
}