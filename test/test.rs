// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

#[macro_use] extern crate log;
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
        info!("test_send_while_not_connected: msg received");
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

    let client_request = vec!(65, 66, 67);
    client.send(client_request).unwrap();

    let server_request = server.recv().unwrap();
    assert_eq!(vec!(65, 66, 67), server_request);

    let server_reply = vec!(67, 66, 65);
    server.send(server_reply).unwrap();

    let client_reply = client.recv().unwrap();

    assert_eq!(vec!(67, 66, 65), client_reply);
}

// this is not working on appveyor
// for a mysterious reason the publisher can send the 8 bytes of the prefix
// but then the next 3 bytes of the body can't be sent : WouldBlock
// Since Pub requires non-blocking send, the operation is not completed
// the bad news is that it means any send on windows require at least one event loop round trip
// and that all dist base protocols will NOT WORK AT ALL !!!
// ie: pub, survey, bus
#[cfg(not(windows))]
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

    thread::sleep(time::Duration::from_millis(500));

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

//#[test]
fn test_bus() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Bus).unwrap();
    let mut client1 = session.create_socket(SocketType::Bus).unwrap();
    let mut client2 = session.create_socket(SocketType::Bus).unwrap();

    server.bind("tcp://127.0.0.1:5464").unwrap();
    client1.connect("tcp://127.0.0.1:5464").unwrap();
    client2.connect("tcp://127.0.0.1:5464").unwrap();

    let sent = vec![65, 66, 67];
    server.send(sent).unwrap();
    let received1 = client1.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received1);
    let received2 = client2.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received2);
}

//#[test]
fn test_survey() {
    let _ = env_logger::init();
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Surveyor).unwrap();
    let mut client1 = session.create_socket(SocketType::Respondent).unwrap();
    let mut client2 = session.create_socket(SocketType::Respondent).unwrap();

    server.bind("tcp://127.0.0.1:5465").unwrap();
    client1.connect("tcp://127.0.0.1:5465").unwrap();
    client2.connect("tcp://127.0.0.1:5465").unwrap();

    let server_survey = vec!(65, 66, 67);
    server.send(server_survey).unwrap();

    let client1_survey = client1.recv().unwrap();
    assert_eq!(vec!(65, 66, 67), client1_survey);

    let client2_survey = client2.recv().unwrap();
    assert_eq!(vec!(65, 66, 67), client2_survey);

    client1.send(vec!(65, 66, 65)).unwrap();
    let server_resp1 = server.recv().unwrap();
    assert_eq!(vec!(65, 66, 65), server_resp1);

    client2.send(vec!(67, 66, 67)).unwrap();
    let server_resp2 = server.recv().unwrap();
    assert_eq!(vec!(67, 66, 67), server_resp2);
}


#[test]
fn test_send_reply_before_send_request() {
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Rep).unwrap();

    server.bind("tcp://127.0.0.1:5466").unwrap();
    server.send(vec!(67, 66, 65)).unwrap_err();
}


#[test]
fn test_recv_reply_before_send_request() {
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Rep).unwrap();
    let mut client = session.create_socket(SocketType::Req).unwrap();

    server.bind("tcp://127.0.0.1:5467").unwrap();
    client.connect("tcp://127.0.0.1:5467").unwrap();

    let err = client.recv().unwrap_err();
    assert_eq!(io::ErrorKind::Other, err.kind());
}

//#[test]
fn test_survey_deadline() {
    let session = Session::new().unwrap();
    let mut server = session.create_socket(SocketType::Surveyor).unwrap();
    let mut client = session.create_socket(SocketType::Respondent).unwrap();
    let timeout = time::Duration::from_millis(150);

    server.set_option(SocketOption::SurveyDeadline(timeout)).unwrap();
    server.bind("tcp://127.0.0.1:5468").unwrap();
    client.connect("tcp://127.0.0.1:5468").unwrap();

    let server_survey = vec!(65, 66, 67);
    server.send(server_survey).unwrap();

    let client_survey = client.recv().unwrap();
    assert_eq!(vec!(65, 66, 67), client_survey);

    thread::sleep(time::Duration::from_millis(200));

    let err = server.recv().unwrap_err();
    assert_eq!(io::ErrorKind::Other, err.kind());
}

//#[test]
//fn test_req_resend() {
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
//}


#[cfg(not(windows))]
#[test]
fn test_ipc() {
    let session = Session::new().unwrap();
    let mut bound = session.create_socket(SocketType::Pair).unwrap();
    let mut connected = session.create_socket(SocketType::Pair).unwrap();

    bound.bind("ipc:///tmp/test_ipc.ipc").unwrap();
    connected.connect("ipc:///tmp/test_ipc.ipc").unwrap();

    connected.send(vec!(65, 66, 67)).unwrap();
    let received = bound.recv().unwrap();
    assert_eq!(vec![65, 66, 67], received);

    bound.send(vec!(67, 66, 65)).unwrap();
    let received = connected.recv().unwrap();
    assert_eq!(vec!(67, 66, 65), received);
}

