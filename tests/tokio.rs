use tokio::{io::AsyncReadExt, net::{TcpListener, TcpStream}};


fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

            loop {
                let (socket, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    process(socket).await;
                });
            }
        })
}

async fn process(mut socket: TcpStream) {
    let mut buf = Vec::new();
    let _ = socket.read(&mut buf).await;
    println!("socket read: {:?}",buf);
}