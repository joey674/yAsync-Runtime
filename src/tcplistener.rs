use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::os::fd::AsRawFd;

use super::*;


pub struct AsyncTcpListener {
    listener: TcpListener,
    eventloop: Arc<EventLoop>,
    waker: Arc<Mutex<Option<Waker>>>,
}
impl AsyncTcpListener {
    pub async fn bind(addr: &'static str, eventloop: Arc<EventLoop>) -> Result<Self,String> {
        let waker_slot = Arc::new(Mutex::new(None::<Waker>));
        let listener_slot = Arc::new(Mutex::new(None));

        let listener_slot_cl = listener_slot.clone();
        let waker_slot_cl = waker_slot.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_nanos(1));/* 模拟绑定操作需要的事件其实listener bind不用异步 但是这里把这个行为改成假异步 */
            let result = TcpListener::bind(addr).map_err(|x| format!("error: tcp listener bind fail,{}",x));

            let mut listener_slot = listener_slot_cl.lock().unwrap();
            *listener_slot = Some(result);

            let mut waker_slot = waker_slot_cl.lock().unwrap();
            if let Some(waker) = waker_slot.take() {
                waker.wake()
            }
        });

        let listener = BindFuture {
            listener_slot,
            waker_slot
        }.await.unwrap();
        listener.set_nonblocking(true).unwrap();

        Ok(AsyncTcpListener {
            listener,
            eventloop,
            waker: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn accept(&self) -> Result<AsyncTcpStream,String> {
        AcceptFuture {
            listener: self,
        }
        .await
    }
}


struct AcceptFuture<'a> {
    listener: &'a AsyncTcpListener
}
impl<'a> Future for AcceptFuture<'a> {
    type Output = Result<AsyncTcpStream,String>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.listener.listener.accept() {
            Ok((stream, _)) => {
                let stream = AsyncTcpStream::new(stream, self.listener.eventloop.clone());
                Poll::Ready(Ok(stream))
            },
            Err(e) if e.kind() == futures::io::ErrorKind::WouldBlock => { /* 这个是future还没完成的信息 由于设置了non blocking，所以会直接返回结果 */
                let mut waker = self.listener.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());

                let listener_fd = self.listener.listener.as_raw_fd();/* 由于设置的是oneshot，所以重新注册 */
                let waker_clone = self.listener.waker.clone();
                let eventloop = self.listener.eventloop.clone();
                eventloop.register(listener_fd, EventType::Readable, move || {
                    if let Some(waker) = waker_clone.lock().unwrap().take() {
                        waker.wake();
                    }
                }).unwrap();

                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(format!("error tcp listener accept: {}",e))),
            
        }
    }
}




struct BindFuture {
    listener_slot: Arc<Mutex< Option<Result<TcpListener,String>> >>,
    waker_slot: Arc<Mutex<Option<Waker>>>,
}

impl Future for BindFuture {
    type Output = Result<TcpListener,String>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut waker_slot = self.waker_slot.lock().unwrap();
        let mut listener_slot = self.listener_slot.lock().unwrap();
        if let Some(listener) = listener_slot.take(){
            Poll::Ready(listener)
        } else {
            *waker_slot = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}


