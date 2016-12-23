// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub use std::time::Duration;
pub use std::thread;
pub use std::io;

pub use scaproust::*;

pub use super::{urls, make_session, make_timeout, sleep_some};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = make_session();
        let mut left = session.create_socket::<Pair>().expect("Failed to create socket !");
        let mut right = session.create_socket::<Pair>().expect("Failed to create socket !");
        let url = urls::ipc::get();
        let timeout = make_timeout();

        left.set_send_timeout(timeout).expect("Failed to set send timeout !");
        left.set_recv_timeout(timeout).expect("Failed to set recv timeout !");

        right.set_send_timeout(timeout).expect("Failed to set send timeout !");
        right.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    }

    it "send a message back and forth" {
        left.bind(&url).unwrap();
        right.connect(&url).unwrap();
        sleep_some();

        let sent_ltr = vec![65, 66, 67];
        left.send(sent_ltr).unwrap();
        let received_ltr = right.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received_ltr);

        /*let sent_rtl = vec![67, 66, 65];
        right.send(sent_rtl).unwrap();
        let received_rtl = left.recv().unwrap();
        assert_eq!(vec![67, 66, 65], received_rtl);*/
    }
}