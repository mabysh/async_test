extern crate mio;
extern crate tokio;
//extern crate bytes;

use mio::unix::EventedFd;
use mio::{Evented, Poll, PollOpt, Ready, Token};

use tokio::prelude::*;
use tokio::reactor::*;

use bytes::*;

#[macro_use]
use futures;

use std::io;
use std::io::{Read, Stdin};
use std::os::unix::io::{AsRawFd, RawFd};
use std::mem;

#[derive(Debug)]
pub struct AsyncStdin {
    inner: Stdin,
    fd: RawFd,
}

impl AsyncStdin {
    pub fn new() -> AsyncStdin {
        let stdin = ::std::io::stdin();
        let fd = AsRawFd::as_raw_fd(&stdin);
        AsyncStdin {
            inner: stdin,
            fd: fd,
        }
    }
}

impl Evented for AsyncStdin {
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

impl Read for AsyncStdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.inner.read_to_string(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_exact(buf)
    }
}

#[derive(Debug)]
pub struct InputStream {
    io: PollEvented2<AsyncStdin>,
    buf: BytesMut,
}

pub fn input_stream() -> InputStream {
    InputStream {
        io: PollEvented2::new(AsyncStdin::new()),
        buf: BytesMut::with_capacity(256),
    }
}

impl Stream for InputStream {
    type Item = BytesMut;
    type Error = io::Error;

    fn poll(&mut self) -> futures::Poll<Option<BytesMut>, io::Error> {
        println!("poll method");
        let readiness = self.io.poll_read_ready(mio::Ready::readable());
        match readiness {
            Ok(r) => match r {
                futures::Async::Ready(i) => {
                    println!("before read");
                    let b = try_ready!(self.io.poll_read(&mut self.buf));
                    if b == 0 && self.buf.len() == 0 {
                        println!("1");
                        return Ok(None.into());
                    }
                    println!("2");
                    return Ok(
                        Some(mem::replace(&mut self.buf, BytesMut::with_capacity(256))).into(),
                    );
                }
                futures::Async::NotReady => {
                    println!("not ready");
                    self.io.clear_read_ready(mio::Ready::readable());
                    return Ok(Async::NotReady);
                }
            },
            Err(e) => return Err(From::from(e)),
        }
        let n = try_ready!(self.io.poll_read(&mut self.buf));
        if n == 0 && self.buf.len() == 0 {
            return Ok(None.into());
        }
        Ok(Some(mem::replace(&mut self.buf, BytesMut::with_capacity(256))).into())
    }
}

use {AsyncRead, AsyncWrite};

/// A future which will copy all data from a reader into a writer.
///
/// Created by the [`copy`] function, this future will resolve to the number of
/// bytes copied or an error if one happens.
///
/// [`copy`]: fn.copy.html
#[derive(Debug)]
pub struct MyCopy<R, W> {
    reader: Option<R>,
    read_done: bool,
    writer: Option<W>,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Box<[u8]>,
}

/// Creates a future which represents copying all the bytes from one object to
/// another.
///
/// The returned future will copy all the bytes read from `reader` into the
/// `writer` specified. This future will only complete once the `reader` has hit
/// EOF and all bytes have been written to and flushed from the `writer`
/// provided.
///
/// On success the number of bytes is returned and the `reader` and `writer` are
/// consumed. On error the error is returned and the I/O objects are consumed as
/// well.
pub fn mycopy<R, W>(reader: R, writer: W) -> MyCopy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    MyCopy {
        reader: Some(reader),
        read_done: false,
        writer: Some(writer),
        amt: 0,
        pos: 0,
        cap: 0,
        buf: Box::new([0; 2048]),
    }
}

impl<R, W> MyCopy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
}

impl<R, W> Stream for MyCopy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    type Item = u64;
    type Error = io::Error;

    fn poll(&mut self) -> futures::Poll<Option<u64>, io::Error> {
        // If our buffer is empty, then we need to read some data to
        // continue.
        println!("poll method");
        if self.pos == self.cap && !self.read_done {
            let reader = self.reader.as_mut().unwrap();
            println!("before read");
            let n = try_ready!(reader.poll_read(&mut self.buf));
            if n == 0 {
                println!("EOF");
                self.read_done = true;
                return Ok(None.into());
            } else {
                println!("read success");
                self.pos = 0;
                self.cap = n;
            }
        }

        // If our buffer has some data, let's write it out!
        while self.pos < self.cap {
            let writer = self.writer.as_mut().unwrap();
            println!("before write");
            let i = try_ready!(writer.poll_write(&self.buf[self.pos..self.cap]));
            if i == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "write zero byte into writer",
                ));
            } else {
                println!("wrote");
                self.pos += i;
                self.amt += i as u64;
            }
        }

        // If we've written al the data and we've seen EOF, flush out the
        // data and finish the transfer.
        // done with the entire transfer.
        println!("before flush");
        try_ready!(self.writer.as_mut().unwrap().poll_flush());
        println!("before ok");
        return Ok(Some(self.amt).into());
    }
}
