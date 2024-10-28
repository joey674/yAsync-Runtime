use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, 
        Poll, 
        Waker},
    thread,
    time::Duration,
    sync::mpsc::{sync_channel, Receiver, SyncSender},
};
use futures::{
    future::{BoxFuture, FutureExt},
    task::{waker_ref, ArcWake},
};


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
    /// 创建一个新的`TimerFuture`，在指定的时间结束后，该`Future`可以完成
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



/// 任务执行器，负责从通道中接收任务然后执行
/// 
/// 
/// 
/// 
/// 
/// 
/// 
/// 
/// 
/// 
/// 
struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}
impl Executor {
    fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            // 获取一个future，若它还没有完成(仍然是Some，不是None)，则对它进行一次poll并尝试完成它
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                // 基于任务自身创建一个 `LocalWaker`
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&*waker);
                // `BoxFuture<T>`是`Pin<Box<dyn Future<Output = T> + Send + 'static>>`的类型别名
                // 通过调用`as_mut`方法，可以将上面的类型转换成`Pin<&mut dyn Future + Send + 'static>`
                if future.as_mut().poll(context).is_pending() {
                    // Future还没执行完，因此将它放回任务中，等待下次被poll
                    *future_slot = Some(future);
                }
            }
        }
    }
}


/// `Spawner`负责创建新的`Future`然后将它发送到任务通道中
/// 
/// 
/// 
/// 
/// 
/// 
/// 
/// 
#[derive(Clone)]
struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}
impl Spawner {
    fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).expect("任务队列已满");
    }
}


/// 一个Future，它可以调度自己(将自己放入任务通道中)，然后等待执行器去`poll`
/// 
/// 
/// 
/// 
/// 
/// 
/// 
struct Task {
    /// 进行中的Future，在未来的某个时间点会被完成
    ///
    /// 按理来说`Mutex`在这里是多余的，因为我们只有一个线程来执行任务。但是由于
    /// Rust并不聪明，它无法知道`Future`只会在一个线程内被修改，并不会被跨线程修改。因此
    /// 我们需要使用`Mutex`来满足这个笨笨的编译器对线程安全的执着。
    ///
    /// 如果是生产级的执行器实现，不会使用`Mutex`，因为会带来性能上的开销，取而代之的是使用`UnsafeCell`
    future: Mutex<Option<BoxFuture<'static, ()>>>,

    /// 可以将该任务自身放回到任务通道中，等待执行器的poll
    task_sender: SyncSender<Arc<Task>>,
}
impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // 通过发送任务到任务管道的方式来实现`wake`，这样`wake`后，任务就能被执行器`poll`
        let cloned = arc_self.clone();
        arc_self
            .task_sender
            .send(cloned)
            .expect("任务队列已满");
    }
}


fn new_executor_and_spawner() -> (Executor, Spawner) {
    // 任务通道允许的最大缓冲数(任务队列的最大长度)
    // 当前的实现仅仅是为了简单，在实际的执行中，并不会这么使用
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { ready_queue }, Spawner { task_sender })
}




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