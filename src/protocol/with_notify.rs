// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc::Sender;

use event_loop_msg::{ SocketNotify };

pub trait WithNotify {
    fn get_notify_sender(&self) -> &Sender<SocketNotify>;

    fn send_notify(&self, evt: SocketNotify) {
        let send_res = self.get_notify_sender().send(evt);

        if send_res.is_err() {
            error!("Failed to send notify to the facade: '{:?}'", send_res.err());
        }
    }
}
