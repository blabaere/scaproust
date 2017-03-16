// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub use std::time::Duration;
pub use std::thread;
pub use std::io;

pub use scaproust::*;

pub use super::{urls, make_session, make_timeout, sleep_some};

fn before_each() -> (Session, Socket, Socket, String) {
    let _ = ::env_logger::init();
    let mut session = make_session();
    let mut req = session.create_socket::<Req>().expect("Failed to create socket !");
    let mut rep = session.create_socket::<Rep>().expect("Failed to create socket !");
    let url = urls::tcp::get();
    let timeout = make_timeout();

    req.set_send_timeout(timeout).expect("Failed to set send timeout !");
    rep.set_send_timeout(timeout).expect("Failed to set send timeout !");
    req.set_recv_timeout(timeout).expect("Failed to set recv timeout !");
    rep.set_recv_timeout(timeout).expect("Failed to set recv timeout !");

    (session, req, rep, url)
}

#[test]
fn send_a_request_and_receive_a_reply() {
    let (session, mut req, mut rep, url) = before_each();

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
    drop(session);
}

#[test]
fn refuse_to_receive_a_reply_before_sending_a_request() {
    let (session, mut req, mut rep, url) = before_each();

    rep.bind(&url).unwrap();
    req.connect(&url).unwrap();

    let not_received = req.recv().unwrap_err();
    assert_eq!(io::ErrorKind::Other, not_received.kind());
    drop(session);
}

#[test]
fn refuse_to_send_a_reply_before_receiving_a_request() {
    let (session, mut req, mut rep, url) = before_each();

    rep.bind(&url).unwrap();
    req.connect(&url).unwrap();

    let not_sent = rep.send(vec![66, 65, 67]).unwrap_err();
    assert_eq!(io::ErrorKind::InvalidData, not_sent.kind());
    drop(session);
}

#[test]
fn resend_the_request_silently_when_the_timeout_is_reached() {
    let (session, mut req, mut rep, url) = before_each();

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
    drop(session);
}
