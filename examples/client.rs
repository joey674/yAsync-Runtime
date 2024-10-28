use std::net::TcpStream;
use std::io::{self, Write, Read};
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
     let num_clients = 10;
     let server_addr = "127.0.0.1:6379";
 
     let mut handles = Vec::new();
     for i in 0..num_clients {
         let handle = thread::spawn(move || {
             match TcpStream::connect(server_addr) {
                 Ok(mut stream) => {
                     println!("Client {} connected to the server!", i);
                    for j in 1.. {  
                        let message = format!("Client {} - Message {}", i, j);
                        if let Err(e) = stream.write_all(message.as_bytes()) {
                            eprintln!("Client {} failed to send: {}", i, e);
                            break;
                        }
                        println!("Client {} sent: {}", i, message);
                        
                        thread::sleep(Duration::from_secs(1));
                    }
                 }
                 Err(e) => eprintln!("Client {} failed to connect: {}", i, e),
             }
         });
 
         handles.push(handle);
     }
 
     for handle in handles {
         handle.join().expect("Thread panicked");
     }
 
     Ok(())
}
