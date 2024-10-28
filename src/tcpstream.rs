use std::{
    io::{ErrorKind, Read}, net::TcpStream, sync::{Arc,Mutex}, task::{Context, Poll, Waker},
    os::fd::AsRawFd,
};
use futures::Future;

use super::*;


pub struct AsyncTcpStream {
    stream: Arc<Mutex<TcpStream>>,
    eventloop: Arc<EventLoop>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl AsyncTcpStream {
    pub fn new(stream : TcpStream, eventloop: Arc<EventLoop>) -> Self{
        stream.set_nonblocking(true).unwrap();/* !!!!!非常重要！！！ 如果忘记设置了 read函数会阻塞运行 后面的future就直接堵死了 */
        Self {
            stream: Arc::new(Mutex::new(stream)),
            eventloop,
            waker: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        let readbuf: Result<Vec<u8>, String> =ReadFuture {
            asyncstream: self
        }.await;

        let readbuf = match readbuf {
            Ok(data) => data,
            Err(e) => return Err(e),
        };

        buf.fill(0);
        let len_to_copy = buf.len().min(readbuf.len());
        buf[..len_to_copy].copy_from_slice(&readbuf[..len_to_copy]);
        Ok(len_to_copy)
    }
}

pub struct ReadFuture<'a> {
    asyncstream: &'a mut AsyncTcpStream,  
}

impl<'a> Future for ReadFuture<'a> {
    type Output = Result<Vec<u8>, String>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        
        let mut buf = vec![0;1024];
        let mut stream = self.asyncstream.stream.lock().unwrap();
        match stream.read(&mut buf) {
            Ok(num) =>{
                match num {
                    n if n > buf.len() =>{
                        /* TODO 没读完 */
                        Poll::Ready(Ok(buf))
                    },
                    0 => {
                        /* TODO 要关闭了 */
                        Poll::Ready(Err(format!("error buf len = 0")))
                        // Poll::Ready(Ok(Vec::new()))
                     },
                    _ => {
                        unsafe {
                            buf.set_len(num);
                        }
                        Poll::Ready(Ok(buf))
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                let mut waker = self.asyncstream.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());

                let listener_fd = (*stream).as_raw_fd();
                let waker_clone = self.asyncstream.waker.clone();
                let eventloop = self.asyncstream.eventloop.clone();
                eventloop.register(listener_fd, EventType::Readable, move || {
                    if let Some(waker) = waker_clone.lock().unwrap().take() {
                        waker.wake();
                    }
                }).unwrap();

                Poll::Pending
            }
            Err(e) =>{
                Poll::Ready(Err(format!("error tcp stream read: {}",e)))
            }
        }
    }
}