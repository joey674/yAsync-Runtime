# yAsync-Runtime
A simple runtime for async I/O simply based on std and epoll.
Without any use of tokio and mio(sure I learn from them, and I build a simple one on my own). Now it works with one working thread and one IO multiplexing thread(muti working thread is a TODO). So now it kind of like node.js. 



# Usage

<!-- Add to your Cargo.toml dependencies:
```
```
-->


Here three main future are developed: 
    -listener::bind;  
    -listener.accept().await; 
    -socket.read(&mut buf).await;
```rust
// example/server.rs
fn main() {
    let (executor, spawner) = new_executor_and_spawner();
    let eventloop = EventLoop::new().unwrap();

    let eventloop_cl = eventloop.clone();
    thread::spawn(move || {
        eventloop_cl.run().unwrap();
    });

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
            println!("message :{:?}",String::from_utf8(buf));
        }
    }
}
``` 
detailed example is in /examples. 

