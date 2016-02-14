// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use global::*;

use event_loop_msg::{ SocketNotify };

use super::with_notify::WithNotify;

pub trait WithoutRecv : WithNotify{

    fn recv(&mut self) {
        let err = other_io_error("recv not supported by protocol");
        let ntf = SocketNotify::MsgNotRecv(err);

        self.send_notify(ntf);
    }
}
