use std::{
    fmt::format, future::Future, net::TcpListener, os::unix::process, pin::Pin, sync::{mpsc::{sync_channel, Receiver, SyncSender}, Arc, Mutex}, task::{Context, 
        Poll, 
        Waker}, thread, time::Duration
};
use futures::{
    future::{BoxFuture, FutureExt},
    task::{waker_ref, ArcWake},
};

use super::*;


/* 
    Future表示一个异步计算，在将来可能会被完成。是一个异步任务Task的返回值；
*/
// trait Future { /* 如果引入future::Future，就是引入此特征 */
//     type Output;
//     fn poll(
//         self: Pin<&mut Self>, 
//         cx: &mut Context<'_>,/* // 这里就是`wake: fn()` 修改为 `cx: &mut Context<'_>`: */
//     ) -> Poll<Self::Output>;/* 意思是返回值是个Poll枚举 */
// }
// enum Poll<T> { /* 如果引入task::Poll,就是引入这个枚举类； 其中Ready中的类型就是Ready会带这这个异步计算的结果返回 */
//     Ready(T),
//     Pending,
// }
pub struct TcpListenerBind {
    listener_slot: Arc<Mutex< Option<Result<TcpListener,String>> >>,
    waker_slot: Arc<Mutex<Option<Waker>>>,
}

impl Future for TcpListenerBind {
    type Output = Result<TcpListener,String>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> { /* cx: &mut Context<'_>在future中的作用就是一个wake函数 如果有人调用wake函数 就会把任务重新放进队列等待执行器执行；记得每次poll完后都要注册新的waker */
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
impl TcpListenerBind {
    pub fn new(addr: &'static str) -> Self {
        let waker_slot = Arc::new(Mutex::new(None::<Waker>));
        let listener_slot = Arc::new(Mutex::new(None));

        let listener_slot_cl = listener_slot.clone();/* 复制一份到新线程 用来填补 当这个异步计算中有listener 就说明该异步计算已经完成 */
        let waker_slot_cl = waker_slot.clone();
        thread::spawn(move || {

            thread::sleep(Duration::from_nanos(10));/* 模拟绑定操作需要的事件其实listener bind不用异步 但是这里把这个行为改成假异步 */
            let result = TcpListener::bind(addr).map_err(|x| format!("error: tcp listener bind fail,{}",x));

            let mut listener_slot = listener_slot_cl.lock().unwrap();
            *listener_slot = Some(result);

            let mut waker_slot = waker_slot_cl.lock().unwrap();
            if let Some(waker) = waker_slot.take() {
                waker.wake()
            }
        });

        TcpListenerBind {
            listener_slot,
            waker_slot
        }
    }
}



#[test]
fn main() {
    let (executor, spawner) = new_executor_and_spawner();
    
    let spawner_cl = spawner.clone();
    spawner.spawn(async move  {
        let listener = TcpListenerBind::new("127.0.0.1:6379").await.unwrap_or_else(|e| panic!("Failed to bind listener: {}", e));
        println!("server is listenning");
        // loop {

            // let (socket, _) = listener.accept().await.unwrap();
            // spawner_cl.spawn(async move {
            //     process().await;
            // });
        // }
    });

    let spawner_cl = spawner.clone();
    spawner.spawn(async move  {
        for i in 1..10 {
            spawner_cl.spawn(async move  {
                println!("howdy! {}",i);
                Timer::new(Duration::new(2, 0)).await;
                println!("done! {}",i);
            });
        }
    });

    executor.run();
}

async fn process() {}