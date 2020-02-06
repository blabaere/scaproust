// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use mio::{Evented, Ready, PollOpt};

pub trait EndpointRegistrar {
    fn register(&mut self, io: &dyn Evented, interest: Ready, opt: PollOpt);
    fn reregister(&mut self, io: &dyn Evented, interest: Ready, opt: PollOpt);
    fn deregister(&mut self, io: &dyn Evented);
}
