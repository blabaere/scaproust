#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io;

use scaproust::{Session, SocketType, Socket};

fn handle_recv_ok(socket: &mut Socket, msg: Vec<u8>) {
    info!("Received a survey: '{:?}' !", msg);

    match socket.send(vec!(67, 66, 65)) {
        Ok(_) => info!("vote sent !"),
        Err(e) => error!("vote NOT sent: '{:?}'", e)
    }
}

fn handle_recv_err(err: io::Error) {
    if err.kind() == io::ErrorKind::TimedOut {
        debug!("Still waiting for a survey ...");
    } else {
        error!("Failed to recv a survey: '{:?}' !", err);
    }
}

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().unwrap();
    let mut socket = session.create_socket(SocketType::Respondent).unwrap();

    socket.connect("tcp://127.0.0.1:5461").unwrap();

    loop {
        match socket.recv() {
            Ok(msg) => handle_recv_ok(&mut socket, msg),
            Err(e) => handle_recv_err(e)
        }
    }
}
