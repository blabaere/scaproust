#[macro_use] extern crate log;
extern crate env_logger;
extern crate scaproust;

use std::time::Duration;
use scaproust::{Session, SocketType, SocketOption};

fn main() {

    env_logger::init().unwrap();
    info!("Logging initialized.");

    let session = Session::new().unwrap();
    let mut resp = session.create_socket(SocketType::Respondent).unwrap();
    let mut surv = session.create_socket(SocketType::Surveyor).unwrap();

    surv.set_option(SocketOption::SurveyDeadline(Duration::from_secs(60))).unwrap();
    resp.bind("tcp://127.0.0.1:5458").unwrap();
    surv.bind("tcp://127.0.0.1:5459").unwrap();

    let device = session.create_bridge_device(surv, resp).unwrap();
    let err = device.run().unwrap_err();

    info!("Device finished with: {:?}", err);
}
