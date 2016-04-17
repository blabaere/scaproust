#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::io::*;
use std::time;
use std::thread;

use scaproust::{Session, SocketType};

const SERVER: &'static str = "server";
const CLIENT: &'static str = "client";
const DATE: &'static str = "DATE";

fn sleep_ms(ms: u64) {
    thread::sleep(time::Duration::from_millis(ms));
}

fn server(url: &str) {
    let session = Session::new().expect("Failed to create session !");
    let mut socket = session.create_socket(SocketType::Surveyor).expect("Failed to create socket !");
    let buffer = From::from(DATE.as_bytes());

    println!("SERVER: SENDING DATE SURVEY REQUEST");
    socket.bind(url).expect("Failed to bind socket !");
    sleep_ms(1000);
    
    socket.send(buffer).expect("Send failed !");

    loop {
        match socket.recv() {
            Ok(vote) => {
                let msg = std::str::from_utf8(&vote).expect("Failed to parse vote msg !");
                println!("SERVER: RECEIVED \"{}\" SURVEY RESPONSE", msg);
            },
            Err(e) => {
                if e.kind() == ErrorKind::TimedOut {
                    break;
                }
            }
        }
    }
}

fn client(url: &str, name: &str) {
    let session = Session::new().expect("Failed to create session !");
    let mut socket = session.create_socket(SocketType::Respondent).expect("Failed to create socket !");

    socket.connect(url).expect("Failed to connect socket !");

    loop {
        let buffer = socket.recv().expect("Recv failed !");
        let survey = std::str::from_utf8(&buffer).expect("Failed to parse survey msg !");

        println!("CLIENT ({}): RECEIVED \"{}\" SURVEY REQUEST", name, survey);
        println!("CLIENT ({}): SENDING DATE SURVEY RESPONSE", name);
        let vote = From::from(name.as_bytes());
        socket.send(vote).expect("Send failed !");
    }
}

fn usage(program: &str) -> ! {
    let _ = writeln!(stderr(), "Usage: {} {}|{} <URL> <ARG> ...", program, SERVER, CLIENT);
    std::process::exit(1)
}

fn main() {
    env_logger::init().unwrap();

    let os_args: Vec<_> = std::env::args().collect();
    let args: Vec<&str> = os_args.iter().map(|x| x.as_ref()).collect();
    let program = args[0];

    if args.len() < 2 {
        usage(program);
    }

    match args[1] {
        SERVER if args.len() == 3 => server(args[2]),
        CLIENT if args.len() == 4 => client(args[2], args[3]),
        _ => usage(program)
    }
}