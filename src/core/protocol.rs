// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::boxed::FnBox;
use std::sync::mpsc::Sender;

use core::EndpointId;
use core::socket::Reply;
use core::endpoint::Pipe;
use core::network::Network;
use core::message::Message;
