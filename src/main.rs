use std::{
    sync::Arc, 
    thread, 
};
use yAsync_Runtime::*;



fn main() {
    let (executor, spawner) = new_executor_and_spawner();
    let eventloop = EventLoop::new().unwrap();

    let eventloop_cl = eventloop.clone();
    thread::spawn(move || {
        eventloop_cl.run().unwrap();
    });

    /* 具体任务例子 */
    let spawner_cl = spawner.clone();
    let eventloop_cl = Arc::new(eventloop.clone());
    spawner.spawn(async move  {
        let listener = AsyncTcpListener::bind("127.0.0.1:6379",eventloop_cl).await.unwrap();
        println!("server is listenning");
        loop {
            let mut socket = listener.accept().await.unwrap();
            println!("new connection incomming");
            spawner_cl.spawn(async move {
                process(&mut socket).await;
            });
        }
    });
    executor.run();
}


async fn process(socket: &mut AsyncTcpStream) {
    loop{
        let mut buf = vec![0;1024];
        if let Ok(num) = socket.read(&mut buf).await{
            unsafe {    
                buf.set_len(num);
            }

            println!("message :{:?}",String::from_utf8(buf));
        } else {
            println!("connection closed or error occured.");
            break;
        }
    }
}