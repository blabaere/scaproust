// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io::Result;
use std::time::Duration;

pub struct Config {
    pub send_timeout: Option<Duration>,
    pub send_priority: u8,
    pub recv_timeout: Option<Duration>,
    pub recv_priority: u8,
    pub retry_ivl: Duration,
    pub retry_ivl_max: Duration
}

pub enum ConfigOption {
    SendTimeout(Duration),
    SendPriority(u8),
    RecvTimeout(Duration),
    RecvPriority(u8),
    RetryIvl(Duration),
    RetryIvlMax(Duration),
}

impl Config {
    pub fn new() -> Config {
        Config {
            send_timeout: None,
            send_priority: 8,
            recv_timeout: None,
            recv_priority: 8,
            retry_ivl: Duration::from_millis(100),
            retry_ivl_max: Duration::from_millis(100)
        }
    }

    fn set(&mut self, cfg_opt: ConfigOption) -> Result<()> {
        Ok(())
    }

}