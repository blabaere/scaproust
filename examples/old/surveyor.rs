#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io::*;

use std::time::Duration;
use scaproust::{Session, SocketType, SocketOption};

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().unwrap();
    let mut socket = session.create_socket(SocketType::Surveyor).unwrap();

    socket.set_option(SocketOption::SurveyDeadline(Duration::from_secs(60))).unwrap();
    socket.connect("tcp://127.0.0.1:5458").unwrap();

    let mut input = String::new();

    stdout().write(b"Hit 'Enter' to send the survey\n").unwrap();
    stdin().read_line(&mut input).unwrap();
    match socket.send(vec!(65, 66, 67)) {
        Ok(_) => info!("survey sent !"),
        Err(e) => error!("survey NOT sent: {} !", e)
    }

    stdout().write(b"Hit 'Enter' to receive the first vote\n").unwrap();
    stdin().read_line(&mut input).unwrap();
    match socket.recv_msg() {
        Ok(msg) => info!("vote received '{:?}' !", msg.get_body()),
        Err(e) => error!("vote NOT received: {} !", e)
    }

    stdout().write(b"Hit 'Enter' to receive the second vote\n").unwrap();
    stdin().read_line(&mut input).unwrap();
    match socket.recv_msg() {
        Ok(msg) => info!("vote received '{:?}' !", msg.get_body()),
        Err(e) => error!("vote NOT received: {} !", e)
    }

    stdout().write(b"Hit 'Enter' to exit ...\n").unwrap();
    stdin().read_line(&mut input).unwrap();

}
