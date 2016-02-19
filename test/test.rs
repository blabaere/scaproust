// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io;
use std::time;
use std::thread;

use scaproust::*;


#[test]
fn test_pipeline_connected_to_bound() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();

    pull.bind("tcp://127.0.0.1:5454").unwrap();
    push.connect("tcp://127.0.0.1:5454").unwrap();

    let sent = vec![65, 66, 67];
    push.send(sent).unwrap();
    let received = pull.recv().unwrap();

    assert_eq!(vec![65, 66, 67], received)
}


#[test]
fn test_pipeline_bound_to_connected() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();

    push.bind("tcp://127.0.0.1:5455").unwrap();
    pull.connect("tcp://127.0.0.1:5455").unwrap();

    let sent = vec![65, 66, 67];
    push.send(sent).unwrap();
    let received = pull.recv().unwrap();

    assert_eq!(vec![65, 66, 67], received)
}


#[test]
fn test_send_while_not_connected() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let timeout = time::Duration::from_millis(250);

    let recver = thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(50));
        pull.connect("tcp://127.0.0.1:5456").unwrap();
        let received = pull.recv().unwrap();
        assert_eq!(vec![65, 66, 67], received)
    });

    push.set_send_timeout(timeout).unwrap();
    push.bind("tcp://127.0.0.1:5456").unwrap();
    push.send(vec![65, 66, 67]).unwrap();
    info!("test_send_while_not_connected: msg sent");

    recver.join().unwrap();
}


#[test]
fn test_send_timeout() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();
    let timeout = time::Duration::from_millis(50);

    push.bind("tcp://127.0.0.1:5457").unwrap();
    push.set_send_timeout(timeout).unwrap();

    let err = push.send(vec![65, 66, 67]).unwrap_err();

    assert_eq!(io::ErrorKind::TimedOut, err.kind());
}


#[test]
fn test_recv_while_not_connected() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();
    let timeout = time::Duration::from_millis(250);

    pull.set_recv_timeout(timeout).unwrap();
    pull.bind("tcp://127.0.0.1:5458").unwrap();

    let sender = thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(50));
        push.connect("tcp://127.0.0.1:5458").unwrap();
        push.send(vec![65, 66, 67]).unwrap();
        thread::sleep(timeout);
    });

    let received = pull.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received);

    sender.join().unwrap();
}


#[test]
fn test_recv_timeout() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();
    let timeout = time::Duration::from_millis(50);

    pull.set_recv_timeout(timeout).unwrap();
    pull.bind("tcp://127.0.0.1:5459").unwrap();
    push.connect("tcp://127.0.0.1:5459").unwrap();

    let err = pull.recv().unwrap_err();

    assert_eq!(io::ErrorKind::TimedOut, err.kind());
}


#[test]
fn test_pair_connected_to_bound() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut bound = session.create_socket(SocketType::Pair).unwrap();
    let mut connected = session.create_socket(SocketType::Pair).unwrap();

    bound.set_recv_timeout(time::Duration::from_millis(250)).unwrap();
    bound.bind("tcp://127.0.0.1:5460").unwrap();

    connected.set_send_timeout(time::Duration::from_millis(250)).unwrap();
    connected.connect("tcp://127.0.0.1:5460").unwrap();

    let sent = vec![65, 66, 67];
    connected.send(sent).unwrap();
    let received = bound.recv().unwrap();

    assert_eq!(vec![65, 66, 67], received)
}


#[test]
fn test_pair_bound_to_connected() {
    let _ = env_logger::init();
    info!("test_pair_bound_to_connected");
    let session = Session::new().unwrap();
    let mut bound = session.create_socket(SocketType::Pair).unwrap();
    let mut connected = session.create_socket(SocketType::Pair).unwrap();

    bound.set_send_timeout(time::Duration::from_millis(250)).unwrap();
    bound.bind("tcp://127.0.0.1:5461").unwrap();

    connected.set_recv_timeout(time::Duration::from_millis(250)).unwrap();
    connected.connect("tcp://127.0.0.1:5461").unwrap();

    let sent = vec![65, 66, 67];
    bound.send(sent).unwrap();
    let received = connected.recv().unwrap();

    assert_eq!(vec![65, 66, 67], received)
}


#[test]
fn test_req_rep() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Rep).unwrap();
    let mut client = session.create_socket(SocketType::Req).unwrap();

    server.bind("tcp://127.0.0.1:5462").unwrap();
    client.connect("tcp://127.0.0.1:5462").unwrap();

    let client_request = vec![65, 66, 67];
    client.send(client_request).unwrap();

    let server_request = server.recv().unwrap();
    assert_eq!(vec![65, 66, 67], server_request);

    let server_reply = vec![67, 66, 65];
    server.send(server_reply).unwrap();

    let client_reply = client.recv().unwrap();

    assert_eq!(vec![67, 66, 65], client_reply);
}

#[test]
fn test_pub_sub() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Pub).unwrap();
    let mut client = session.create_socket(SocketType::Sub).unwrap();
    let timeout = time::Duration::from_millis(50);

    server.bind("tcp://127.0.0.1:5463").unwrap();
    client.connect("tcp://127.0.0.1:5463").unwrap();
    client.set_recv_timeout(timeout).unwrap();
    client.set_option(SocketOption::Subscribe("A".to_string())).unwrap();
    client.set_option(SocketOption::Subscribe("B".to_string())).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    server.send(vec![65, 66, 67]).unwrap();
    let received_a = client.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received_a);

    server.send(vec![66, 65, 67]).unwrap();
    let received_b = client.recv().unwrap();
    assert_eq!(vec![66, 65, 67], received_b);

    server.send(vec![67, 66, 65]).unwrap();
    let not_received_c = client.recv().unwrap_err();
    assert_eq!(io::ErrorKind::TimedOut, not_received_c.kind());
}

#[test]
fn test_bus() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Bus).unwrap();
    let mut client1 = session.create_socket(SocketType::Bus).unwrap();
    let mut client2 = session.create_socket(SocketType::Bus).unwrap();
    let timeout = time::Duration::from_millis(50);

    server.bind("tcp://127.0.0.1:5464").unwrap();
    client1.connect("tcp://127.0.0.1:5464").unwrap();
    client2.connect("tcp://127.0.0.1:5464").unwrap();
    client1.set_recv_timeout(timeout).unwrap();
    client2.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(150));

    let sent = vec![65, 66, 67];
    server.send(sent).expect("Server should have send a msg");
    let received1 = client1.recv().expect("Client #1 should have received the msg");
    assert_eq!(vec![65, 66, 67], received1);
    let received2 = client2.recv().expect("Client #2 should have received the msg");
    assert_eq!(vec![65, 66, 67], received2);
}

#[test]
fn test_survey() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Surveyor).unwrap();
    let mut client1 = session.create_socket(SocketType::Respondent).unwrap();
    let mut client2 = session.create_socket(SocketType::Respondent).unwrap();
    let timeout = time::Duration::from_millis(50);

    server.bind("tcp://127.0.0.1:5465").unwrap();
    client1.connect("tcp://127.0.0.1:5465").unwrap();
    client2.connect("tcp://127.0.0.1:5465").unwrap();
    client1.set_recv_timeout(timeout).unwrap();
    client2.set_recv_timeout(timeout).unwrap();
    server.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(500));

    let server_survey = vec![65, 66, 67];
    server.send(server_survey).expect("Server should have send a survey");

    let client1_survey = client1.recv().expect("Client #1 should have received the survey");
    assert_eq!(vec![65, 66, 67], client1_survey);

    let client2_survey = client2.recv().expect("Client #2 should have received the survey");
    assert_eq!(vec![65, 66, 67], client2_survey);

    client1.send(vec![65, 66, 65]).expect("Client #1 should have send a vote");
    let server_resp1 = server.recv().expect("Server should have received the vote from client #1");
    assert_eq!(vec![65, 66, 65], server_resp1);

    client2.send(vec![67, 66, 67]).expect("Client #2 should have send a vote");
    let server_resp2 = server.recv().expect("Server should have received the vote from client #2");
    assert_eq!(vec![67, 66, 67], server_resp2);
}


#[test]
fn test_send_reply_before_send_request() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Rep).unwrap();

    server.bind("tcp://127.0.0.1:5466").unwrap();
    server.send(vec![67, 66, 65]).unwrap_err();
}


#[test]
fn test_recv_reply_before_send_request() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Rep).unwrap();
    let mut client = session.create_socket(SocketType::Req).unwrap();

    server.bind("tcp://127.0.0.1:5467").unwrap();
    client.connect("tcp://127.0.0.1:5467").unwrap();

    let err = client.recv().unwrap_err();
    assert_eq!(io::ErrorKind::Other, err.kind());
}

#[test]
fn test_survey_deadline() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Surveyor).unwrap();
    let mut client = session.create_socket(SocketType::Respondent).unwrap();
    let timeout = time::Duration::from_millis(50);
    let deadline = time::Duration::from_millis(150);

    server.set_option(SocketOption::SurveyDeadline(deadline)).unwrap();
    server.bind("tcp://127.0.0.1:5468").unwrap();
    client.connect("tcp://127.0.0.1:5468").unwrap();
    server.set_recv_timeout(timeout).unwrap();
    server.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(500));

    let server_survey = vec![65, 66, 67];
    server.send(server_survey).unwrap();

    let client_survey = client.recv().unwrap();
    assert_eq!(vec![65, 66, 67], client_survey);

    thread::sleep(time::Duration::from_millis(200));

    let err = server.recv().unwrap_err();
    assert_eq!(io::ErrorKind::Other, err.kind());
}

// #[test]
// fn test_req_resend() {
//    let session = Session::new().unwrap();
//    let mut server = session.create_socket(SocketType::Rep).unwrap();
//    let mut client = session.create_socket(SocketType::Req).unwrap();
//    let timeout = time::Duration::from_millis(300);
//    let resend_ivl = time::Duration::from_millis(150);
//
//    server.bind("tcp://127.0.0.1:5469").unwrap();
//    client.set_recv_timeout(timeout).unwrap();
//    client.set_option(SocketOption::ResendInterval(resend_ivl)).unwrap();
//    client.connect("tcp://127.0.0.1:5469").unwrap();
//
//    let client_request = vec!(65, 66, 67);
//    client.send(client_request).unwrap();
//
//    let server_request = server.recv().unwrap();
//    assert_eq!(vec!(65, 66, 67), server_request);
//
//    ::std::thread::sleep_ms(200);
//    // the request should have been resent at this point, so we can receive it again !
//
//    let server_request2 = server.recv().unwrap();
//    assert_eq!(vec!(65, 66, 67), server_request2);
//
//    server.send(vec!(69, 69, 69)).unwrap();
//
//    let client_reply = client.recv().unwrap();
//
//    assert_eq!(vec!(69, 69, 69), client_reply);
// }


#[cfg(not(windows))]
#[test]
fn test_ipc() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut bound = session.create_socket(SocketType::Pair).unwrap();
    let mut connected = session.create_socket(SocketType::Pair).unwrap();

    bound.bind("ipc:///tmp/test_ipc.ipc").unwrap();
    connected.connect("ipc:///tmp/test_ipc.ipc").unwrap();

    connected.send(vec![65, 66, 67]).unwrap();
    let received = bound.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received);

    bound.send(vec![67, 66, 65]).unwrap();
    let received = connected.recv().unwrap();
    assert_eq!(vec![67, 66, 65], received);
}

#[test]
fn test_device_bus() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Bus).unwrap();
    let mut client1 = session.create_socket(SocketType::Bus).unwrap();
    let mut client2 = session.create_socket(SocketType::Bus).unwrap();
    let timeout = time::Duration::from_millis(50);

    server.bind("tcp://127.0.0.1:5470").unwrap();
    client1.connect("tcp://127.0.0.1:5470").unwrap();
    client2.connect("tcp://127.0.0.1:5470").unwrap();
    client1.set_send_timeout(timeout).unwrap();
    client2.set_send_timeout(timeout).unwrap();
    client1.set_recv_timeout(timeout).unwrap();
    client2.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(500));

    let device = session.create_relay_device(server).unwrap();
    let device_thread = thread::spawn(move || device.run());

    client1.send(vec![65, 66, 67]).unwrap();
    let received = client2.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received);

    let err = client1.recv().unwrap_err();
    assert_eq!(io::ErrorKind::TimedOut, err.kind());

    drop(session);
    device_thread.join().unwrap().unwrap_err();
}

#[test]
fn test_device_pipeline() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut d_push = session.create_socket(SocketType::Push).unwrap();
    let mut d_pull = session.create_socket(SocketType::Pull).unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let timeout = time::Duration::from_millis(50);

    d_push.bind("tcp://127.0.0.1:5471").unwrap();
    d_pull.bind("tcp://127.0.0.1:5472").unwrap();

    push.connect("tcp://127.0.0.1:5472").unwrap();
    pull.connect("tcp://127.0.0.1:5471").unwrap();

    push.set_send_timeout(timeout).unwrap();
    pull.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(500));

    let device = session.create_bridge_device(d_pull, d_push).unwrap();
    let device_thread = thread::spawn(move || device.run());

    push.send(vec![65, 66, 67]).unwrap();
    let received = pull.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received);

    let err = pull.recv().unwrap_err();
    assert_eq!(io::ErrorKind::TimedOut, err.kind());

    drop(session);
    device_thread.join().unwrap().unwrap_err();
}

#[test]
fn check_readable_pipe_is_used_for_recv() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let mut push1 = session.create_socket(SocketType::Push).unwrap();
    let mut push2 = session.create_socket(SocketType::Push).unwrap();
    let mut push3 = session.create_socket(SocketType::Push).unwrap();
    let timeout = time::Duration::from_millis(50);

    pull.bind("tcp://127.0.0.1:5473").unwrap();
    push1.connect("tcp://127.0.0.1:5473").unwrap();
    push2.connect("tcp://127.0.0.1:5473").unwrap();
    push3.connect("tcp://127.0.0.1:5473").unwrap();

    thread::sleep(time::Duration::from_millis(250));

    push1.set_send_timeout(timeout).unwrap();
    push2.set_send_timeout(timeout).unwrap();
    push3.set_send_timeout(timeout).unwrap();
    pull.set_recv_timeout(timeout).unwrap();

    // Make sure the socket will try to read from the correct pipe
    push2.send(vec![65, 66, 67]).unwrap();
    let received1 = pull.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received1);

    // Make sure the socket will try to read from the same correct pipe
    push2.send(vec![67, 66, 65]).unwrap();
    let received2 = pull.recv().unwrap();
    assert_eq!(vec![67, 66, 65], received2);

    // Make sure the socket will try to read from the new correct pipe
    push1.send(vec![66, 67, 65]).unwrap();
    let received3 = pull.recv().unwrap();
    assert_eq!(vec![66, 67, 65], received3);

    // Make sure the socket will try to read from the newest correct pipe
    push3.send(vec![66, 67, 68]).unwrap();
    let received3 = pull.recv().unwrap();
    assert_eq!(vec![66, 67, 68], received3);
}

#[test]
fn check_readable_pipe_is_used_for_recv_when_receiving_two_messages_in_one_event() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut pull = session.create_socket(SocketType::Pull).unwrap();
    let mut push = session.create_socket(SocketType::Push).unwrap();
    let timeout = time::Duration::from_millis(50);

    pull.bind("tcp://127.0.0.1:5474").unwrap();
    push.connect("tcp://127.0.0.1:5474").unwrap();

    push.set_send_timeout(timeout).unwrap();
    pull.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    push.send(vec![65, 66, 67]).expect("First push failed");
    push.send(vec![67, 66, 65]).expect("Second push failed");

    let received1 = pull.recv().expect("First pull failed");
    assert_eq!(vec![65, 66, 67], received1);

    thread::sleep(time::Duration::from_millis(1000));

    let received2 = pull.recv().expect("Second pull failed");
    assert_eq!(vec![67, 66, 65], received2);
}

#[test]
fn device_req_rep() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut d_req = session.create_socket(SocketType::Req).unwrap();
    let mut d_rep = session.create_socket(SocketType::Rep).unwrap();
    let mut req = session.create_socket(SocketType::Req).unwrap();
    let mut rep = session.create_socket(SocketType::Rep).unwrap();
    let timeout = time::Duration::from_millis(50);

    d_req.bind("tcp://127.0.0.1:5475").unwrap();
    d_rep.bind("tcp://127.0.0.1:5476").unwrap();

    req.connect("tcp://127.0.0.1:5476").unwrap();
    rep.connect("tcp://127.0.0.1:5475").unwrap();

    req.set_send_timeout(timeout).unwrap();
    req.set_recv_timeout(timeout).unwrap();
    rep.set_send_timeout(timeout).unwrap();
    rep.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    let device = session.create_bridge_device(d_rep, d_req).unwrap();
    let device_thread = thread::spawn(move || device.run());

    req.send(vec![65, 66, 67]).unwrap();
    let request = rep.recv().unwrap();
    assert_eq!(vec![65, 66, 67], request);

    rep.send(vec![99, 66, 88]).unwrap();
    let reply = req.recv().unwrap();
    assert_eq!(vec![99, 66, 88], reply);

    drop(session);
    device_thread.join().unwrap().unwrap_err();
}

#[test]
fn device_surv_resp_with_sequence_reply() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut d_surv = session.create_socket(SocketType::Surveyor).unwrap();
    let mut d_resp = session.create_socket(SocketType::Respondent).unwrap();
    let mut surv = session.create_socket(SocketType::Surveyor).unwrap();
    let mut resp1 = session.create_socket(SocketType::Respondent).unwrap();
    let mut resp2 = session.create_socket(SocketType::Respondent).unwrap();
    let timeout = time::Duration::from_millis(250);

    d_surv.bind("tcp://127.0.0.1:5477").unwrap();
    d_resp.bind("tcp://127.0.0.1:5478").unwrap();

    surv.connect("tcp://127.0.0.1:5478").unwrap();
    resp1.connect("tcp://127.0.0.1:5477").unwrap();
    resp2.connect("tcp://127.0.0.1:5477").unwrap();

    surv.set_send_timeout(timeout).unwrap();
    surv.set_recv_timeout(timeout).unwrap();
    resp1.set_send_timeout(timeout).unwrap();
    resp1.set_recv_timeout(timeout).unwrap();
    resp2.set_send_timeout(timeout).unwrap();
    resp2.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    let device = session.create_bridge_device(d_surv, d_resp).unwrap();
    let device_thread = thread::spawn(move || device.run());

    thread::sleep(timeout);

    surv.send(vec![65, 66, 67]).unwrap();
    let recv_question1 = resp1.recv().unwrap();
    assert_eq!(vec![65, 66, 67], recv_question1);
    let recv_question2 = resp2.recv().unwrap();
    assert_eq!(vec![65, 66, 67], recv_question2);

    resp1.send(vec![99, 66, 88]).unwrap();
    let answer1 = surv.recv().unwrap();
    assert_eq!(vec![99, 66, 88], answer1);

    resp2.send(vec![99, 66, 00]).unwrap();
    let answer2 = surv.recv().unwrap();
    assert_eq!(vec![99, 66, 00], answer2);

    drop(session);
    device_thread.join().unwrap().unwrap_err();
}

#[test]
fn device_pair_left_to_right() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut d_left = session.create_socket(SocketType::Pair).unwrap();
    let mut d_right = session.create_socket(SocketType::Pair).unwrap();
    let mut left = session.create_socket(SocketType::Pair).unwrap();
    let mut right = session.create_socket(SocketType::Pair).unwrap();
    let timeout = time::Duration::from_millis(50);

    d_left.bind("tcp://127.0.0.1:5479").unwrap();
    d_right.bind("tcp://127.0.0.1:5480").unwrap();

    left.connect("tcp://127.0.0.1:5480").unwrap();
    right.connect("tcp://127.0.0.1:5479").unwrap();

    left.set_send_timeout(timeout).unwrap();
    left.set_recv_timeout(timeout).unwrap();
    right.set_send_timeout(timeout).unwrap();
    right.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    let device = session.create_bridge_device(d_left, d_right).unwrap();
    let device_thread = thread::spawn(move || device.run());

    left.send(vec![65, 66, 67]).unwrap();
    let request = right.recv().unwrap();
    assert_eq!(vec![65, 66, 67], request);

    right.send(vec![99, 66, 88]).unwrap();
    let reply = left.recv().unwrap();
    assert_eq!(vec![99, 66, 88], reply);

    drop(session);
    device_thread.join().unwrap().unwrap_err();
}

#[test]
fn device_pair_right_to_left() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut d_left = session.create_socket(SocketType::Pair).unwrap();
    let mut d_right = session.create_socket(SocketType::Pair).unwrap();
    let mut left = session.create_socket(SocketType::Pair).unwrap();
    let mut right = session.create_socket(SocketType::Pair).unwrap();
    let timeout = time::Duration::from_millis(50);

    d_left.bind("tcp://127.0.0.1:5481").unwrap();
    d_right.bind("tcp://127.0.0.1:5482").unwrap();

    left.connect("tcp://127.0.0.1:5482").unwrap();
    right.connect("tcp://127.0.0.1:5481").unwrap();

    left.set_send_timeout(timeout).unwrap();
    left.set_recv_timeout(timeout).unwrap();
    right.set_send_timeout(timeout).unwrap();
    right.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    let device = session.create_bridge_device(d_left, d_right).unwrap();
    let device_thread = thread::spawn(move || device.run());

    right.send(vec![65, 66, 67]).unwrap();
    let request = left.recv().unwrap();
    assert_eq!(vec![65, 66, 67], request);

    left.send(vec![99, 66, 88]).unwrap();
    let reply = right.recv().unwrap();
    assert_eq!(vec![99, 66, 88], reply);

    drop(session);
    device_thread.join().unwrap().unwrap_err();
}


#[test]
fn device_surv_resp_with_parallel_reply() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut d_surv = session.create_socket(SocketType::Surveyor).unwrap();
    let mut d_resp = session.create_socket(SocketType::Respondent).unwrap();
    let mut surv = session.create_socket(SocketType::Surveyor).unwrap();
    let mut resp1 = session.create_socket(SocketType::Respondent).unwrap();
    let mut resp2 = session.create_socket(SocketType::Respondent).unwrap();
    let timeout = time::Duration::from_millis(250);

    d_surv.bind("tcp://127.0.0.1:5483").unwrap();
    d_resp.bind("tcp://127.0.0.1:5484").unwrap();

    surv.connect("tcp://127.0.0.1:5484").unwrap();
    resp1.connect("tcp://127.0.0.1:5483").unwrap();
    resp2.connect("tcp://127.0.0.1:5483").unwrap();

    surv.set_send_timeout(timeout).unwrap();
    surv.set_recv_timeout(timeout).unwrap();
    resp1.set_send_timeout(timeout).unwrap();
    resp1.set_recv_timeout(timeout).unwrap();
    resp2.set_send_timeout(timeout).unwrap();
    resp2.set_recv_timeout(timeout).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    let device = session.create_bridge_device(d_surv, d_resp).unwrap();
    let device_thread = thread::spawn(move || device.run());

    thread::sleep(timeout);

    surv.send(vec![65, 66, 67]).unwrap();
    let recv_question1 = resp1.recv().unwrap();
    assert_eq!(vec![65, 66, 67], recv_question1);
    let recv_question2 = resp2.recv().unwrap();
    assert_eq!(vec![65, 66, 67], recv_question2);

    resp1.send(vec![99, 66, 88]).unwrap();
    resp2.send(vec![99, 66, 00]).unwrap();

    let answer1 = surv.recv().unwrap();
    assert_eq!(vec![99, 66, 88], answer1);

    let answer2 = surv.recv().unwrap();
    assert_eq!(vec![99, 66, 00], answer2);

    drop(session);
    device_thread.join().unwrap().unwrap_err();
}

#[test]
fn sub_can_skip_crap_and_keep_crop() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Pub).unwrap();
    let mut client = session.create_socket(SocketType::Sub).unwrap();
    let timeout = time::Duration::from_millis(50);

    server.bind("tcp://127.0.0.1:5485").unwrap();
    client.connect("tcp://127.0.0.1:5485").unwrap();
    client.set_recv_timeout(timeout).unwrap();
    client.set_option(SocketOption::Subscribe("A".to_string())).unwrap();
    client.set_option(SocketOption::Subscribe("B".to_string())).unwrap();

    thread::sleep(time::Duration::from_millis(250));

    server.send(vec![99, 99, 99]).unwrap();
    server.send(vec![65, 66, 67]).unwrap();

    let received = client.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received);
}

