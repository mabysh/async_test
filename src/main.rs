extern crate bytes;
#[macro_use]
extern crate futures;
extern crate mio;
extern crate tokio;
#[macro_use]
extern crate tokio_io;
extern crate nonblock;

mod input_stream;

use mio::unix::EventedFd;
use mio::{Evented, Poll, PollOpt, Ready, Token};

use tokio::reactor::PollEvented2;
use tokio::prelude::*;
use tokio_io::codec::*;
use tokio_io::io::read;

use futures::sync::mpsc::*;

use std::io;
use std::fmt;
use std::io::{Stdout, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::thread;
use std::time::Duration;
use std::str;
use std::string::*;

use input_stream::*;

use bytes::*;

fn main() {
    let in_e = PollEvented2::new(AsyncStdin::new());
    let out_e = PollEvented2::new(AsyncStdout::new());
    let stream_one = input_stream()
        .map(|val| {
            let slice = str::from_utf8(&val).unwrap();
            let s = String::from(slice);
            s
        })
        .map_err(|e| eprintln!("Error"));
    let stream_two = tick()
        .map(|x| String::from("TICK"))
        .map_err(|e| eprintln!("Error"));
    let merged = stream_one.select(stream_two).for_each(|v| {
        println!("{}", v);
        Ok(())
    });
    //    let first = stream_one.for_each(|val| {
    //        println!("{}{}", val, val);
    //        Ok(())
    //    });
    //    let fut = mycopy(in_e, out_e)
    //        .for_each(|i| Ok(()))
    //        .map_err(|e| eprintln!("{}", e));
    //    let fut = FramedRead::new(in_e, BytesCodec::new())
    //        .for_each(|v| {
    //            println!("{:?}", v);
    //            Ok(())
    //        })
    //        .map_err(|e| eprintln!("{}", e));
    tokio::run(merged);
}

fn tick() -> Receiver<i32> {
    let (mut tx, rx) = channel::<i32>(1);
    thread::spawn(move || loop {
        match tx.try_send(0) {
            Ok(()) => {
                tx = tx.clone();
            }
            Err(_e) => {
                println!("Error");
            }
        }
        thread::sleep(Duration::from_millis(1000));
    });
    rx
}
struct AsyncStdout {
    inner: Stdout,
    fd: RawFd,
}

impl AsyncStdout {
    fn new() -> AsyncStdout {
        let stdout = std::io::stdout();
        let fd = AsRawFd::as_raw_fd(&stdout);
        AsyncStdout {
            inner: stdout,
            fd: fd,
        }
    }
}

impl Evented for AsyncStdout {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.fd).deregister(poll)
    }
}

impl Write for AsyncStdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.write_all(buf)
    }
    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()> {
        self.inner.write_fmt(args)
    }
}
