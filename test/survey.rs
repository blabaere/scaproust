// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
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
        let mut surv = session.create_socket::<Surveyor>().expect("Failed to create socket !");
        let mut resp1 = session.create_socket::<Respondent>().expect("Failed to create socket !");
        let mut resp2 = session.create_socket::<Respondent>().expect("Failed to create socket !");
        let url = urls::tcp::get();
        let timeout = make_timeout();

        surv.set_recv_timeout(timeout).expect("Failed to set recv timeout !");

        resp1.set_send_timeout(timeout).expect("Failed to set recv timeout !");
        resp1.set_recv_timeout(timeout).expect("Failed to set recv timeout !");

        resp2.set_send_timeout(timeout).expect("Failed to set recv timeout !");
        resp2.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    }

    it "send a survey and receive several responses" {
        surv.bind(&url).unwrap();
        resp1.connect(&url).unwrap();
        resp2.connect(&url).unwrap();

        sleep_some();

        let sent_survey = vec![65, 66, 67];
        surv.send(sent_survey).expect("Surveyor should have sent a survey");
        let received_survey1 = resp1.recv().expect("Respondent 1 should have received a survey");
        let received_survey2 = resp2.recv().expect("Respondent 2 should have received a survey");
        assert_eq!(vec![65, 66, 67], received_survey1);
        assert_eq!(vec![65, 66, 67], received_survey2);

        let sent_response1 = vec![66, 67, 65];
        let sent_response2 = vec![65, 67, 66];
        resp1.send(sent_response1).expect("Respondent 1 should have sent a response");
        resp2.send(sent_response2).expect("Respondent 2 should have sent a response");

        surv.recv().expect("Surveyor should have received response #1");
        surv.recv().expect("Surveyor should have received response #2");
    }

    it "refuse to receive a response before sending a survey" {
        surv.bind(&url).unwrap();
        resp1.connect(&url).unwrap();
        resp2.connect(&url).unwrap();

        let not_received = surv.recv().unwrap_err();
        assert_eq!(io::ErrorKind::Other, not_received.kind());
    }

    it "refuse to send a response before receiving a survey" {
        surv.bind(&url).unwrap();
        resp1.connect(&url).unwrap();
        resp2.connect(&url).unwrap();

        let not_sent = resp1.send(vec![66, 65, 67]).unwrap_err();
        assert_eq!(io::ErrorKind::Other, not_sent.kind());
    }
}