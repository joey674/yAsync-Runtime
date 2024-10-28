use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, 
        Poll, 
        Waker},
    thread,
    time::Duration,
};
use super::*;


pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}
struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}
impl Future for TimerFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl TimerFuture {
    /// 创建一个新的`Timer`，在指定的时间结束后，该`Future`可以完成
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        // 创建新线程
        let thread_shared_state = shared_state.clone();
        thread::spawn(move || {
            // 睡眠指定时间实现计时功能
            thread::sleep(duration);
            let mut shared_state = thread_shared_state.lock().unwrap();
            // 通知执行器定时器已经完成，可以继续`poll`对应的`Future`了
            shared_state.completed = true;
            if let Some(waker) = shared_state.waker.take() {
                waker.wake()
            }
        });

        TimerFuture { shared_state }
    }
}




#[test]
fn main() {
    let (executor, spawner) = new_executor_and_spawner();

    for i in 1..10 {
        let spawner = spawner.clone();
        spawner.spawn(async move  {
            process(i).await;
        });
    }

    drop(spawner);/* 执行队列没有任务 可能是还没注册，也可能是所有程序已经跑完了。如果是暂时空的，执行器不知道。所以这里主动drop spawner，就告诉执行器是程序跑完了而不是执行队列暂时为空 */
    executor.run();
}

async fn process(key: usize){
    println!("howdy! {}",key);
    // 创建定时器Future，并等待它完成
    TimerFuture::new(Duration::new(2, 0)).await;
    println!("done! {}",key);
}