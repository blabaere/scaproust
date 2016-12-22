// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::{Arc, Mutex, Condvar, TryLockError};

pub fn mailbox<T>() -> (Sender<T>, Receiver<T>) {
    let mailbox = Mailbox::default();
    let sender_cell = Arc::new(mailbox);
    let receiver_cell = sender_cell.clone();

    (Sender {cell: sender_cell}, Receiver {cell: receiver_cell})
}

pub struct Sender<T> {
    cell: Arc<Mailbox<T>>
}

impl<T> Sender<T> {
    pub fn try_send(&self, value: T) -> Result<(), T> {
        loop {

            match self.cell.value.try_lock() {
                Ok(mut guard) => {
                    if guard.is_some() {
                        return Err(value);
                    } else {
                        *guard = Some(value);

                        self.cell.monitor.notify_one();

                        return Ok(());
                    }

                },
                Err(TryLockError::Poisoned(_)) => return Err(value),
                Err(TryLockError::WouldBlock) => continue,
            }

        }
    }
}

pub struct Receiver<T> {
    cell: Arc<Mailbox<T>>
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<T, ()> {
        match self.cell.value.lock() {
            Ok(mut guard) => {
                while guard.is_none() {
                    guard = match self.cell.monitor.wait(guard) {
                        Ok(g) => g,
                        Err(_) => return Err(()),
                    }
                }

                guard.take().ok_or(())
            },
            Err(_) => Err(()),
        }
    }

    pub fn try_recv(&self) -> Result<T, ()> {
        match self.cell.value.lock() {
            Ok(mut guard) => guard.take().ok_or(()),
            Err(_)        => Err(()),
        }
    }
}

struct Mailbox<T> {
    value: Mutex<Option<T>>,
    monitor: Condvar
}

impl<T> Default for Mailbox<T> {
    fn default() -> Mailbox<T> {
        Mailbox {
            value: Mutex::new(None),
            monitor: Condvar::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;

    #[test]
    fn when_mailbox_is_empty_send_accepts_the_value() {
        let (tx, _) = mailbox();

        tx.try_send(()).unwrap();
    }

    #[test]
    fn when_mailbox_is_full_send_rejects_the_value() {
        let (tx, _) = mailbox();

        tx.try_send(()).unwrap();
        tx.try_send(()).unwrap_err();
    }

    #[test]
    fn mailbox_can_be_used_accross_threads() {
       let (tx, rx) = mailbox();
       let sender_thread = thread::spawn(move || {
            tx.try_send(true)
       });
       let receiver_thread = thread::spawn(move || {
            rx.recv().unwrap()
       });

       let send_status = sender_thread.join().unwrap();
       let recv_status = receiver_thread.join().unwrap();

       assert!(send_status.is_ok());
       assert!(recv_status);
    }

    #[test]
    fn when_mailbox_is_empty_recv_fails() {
        let (_, rx) = mailbox::<String>();

        rx.try_recv().unwrap_err();
    }

    #[test]
    fn when_mailbox_is_full_recv_returns_the_value() {
        let (tx, rx) = mailbox();

        tx.try_send(()).unwrap();
        rx.try_recv().unwrap();
    }
}