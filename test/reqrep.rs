// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub use std::time::Duration;
pub use std::thread;
pub use std::io;

pub use scaproust::*;

pub use super::urls;
pub use super::{sleep_some, make_timeout};

describe! can {

    before_each {
        let _ = ::env_logger::init();
        let mut session = SessionBuilder::build().expect("Failed to create session !");
        let mut req = session.create_socket::<Req>().expect("Failed to create socket !");
        let mut rep = session.create_socket::<Rep>().expect("Failed to create socket !");
        let url = urls::tcp::get();
        let timeout = make_timeout();

        req.set_send_timeout(timeout).expect("Failed to set send timeout !");
        rep.set_send_timeout(timeout).expect("Failed to set send timeout !");
        req.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
        rep.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    }

    it "send a request and receive a reply" {
        rep.bind(&url).unwrap();
        req.connect(&url).unwrap();

        let sent_request = vec![65, 66, 67];
        req.send(sent_request).unwrap();
        let received_request = rep.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received_request);

        let sent_reply = vec![66, 65, 67];
        rep.send(sent_reply).unwrap();
        let received_reply = req.recv().unwrap();
        assert_eq!(vec![66, 65, 67], received_reply);
    }

    it "refuse to receive a reply before sending a request" {
        rep.bind(&url).unwrap();
        req.connect(&url).unwrap();

        let not_received = req.recv().unwrap_err();
        assert_eq!(io::ErrorKind::Other, not_received.kind());
    }

    it "refuse to send a reply before receiving a request" {
        rep.bind(&url).unwrap();
        req.connect(&url).unwrap();

        let not_sent = rep.send(vec![66, 65, 67]).unwrap_err();
        assert_eq!(io::ErrorKind::Other, not_sent.kind());
    }

    it "resend the request silently when the timeout is reached" {
        let resend_ivl = Duration::from_secs(1);
        rep.bind(&url).unwrap();
        req.connect(&url).unwrap();
        req.set_option(ConfigOption::ReqResendIvl(resend_ivl)).unwrap();

        let sent_request = vec![65, 66, 67];
        req.send(sent_request).unwrap();

        let received_request1 = rep.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received_request1);

        // let's ignore that request and pretend we don't care
        thread::sleep(resend_ivl);
        thread::sleep(Duration::from_millis(100));

        let received_request2 = rep.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received_request2);

        let sent_reply = vec![66, 65, 67];
        rep.send(sent_reply).unwrap();
        let received_reply = req.recv().unwrap();
        assert_eq!(vec![66, 65, 67], received_reply);
    }
}