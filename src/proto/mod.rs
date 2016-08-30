// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub mod push;
pub mod pull;

mod priolist;

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use core::{EndpointId, Message};
use core::socket::Reply;
use core::endpoint::Pipe;
use core::context::{Context, Scheduled};
use self::priolist::Priolist;

pub const PUSH: u16 = (5 * 16);
pub const PULL: u16 = (5 * 16) + 1;

pub type Timeout = Option<Scheduled>;
