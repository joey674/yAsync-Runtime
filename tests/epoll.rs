/* 
    整个程序的流程就是，
    把listener注册进epoll，然后监听；对于listener来说，新的stream连接就是触发事件；
    然后把stream也注册进epoll中监听，有新的可读内容就是触发事件。 通过设置不同的key来区分事件是什么：是listenner的新连接，还是已经注册的stream的新可读内容。

    三个东西：fd，flag， key
    fd 文件标识符：专门给epoll用来提醒对应是否有事件触发的；
    flag 事件状态：用来给特定事件的状态进行修改的；如果把flag设置成读取等等， 然后放进epoll_event里， 就告诉epoll我要监听这个fd上特定的事件；
    key 事件标识符：用来表示特定事件的用户定义的标识符。当epoll返回一系列发生的感兴趣的事件时，我们可以通过event.key找到那个特定的事件

    所以对于我们来说 自定义索引是key；对于epoll来说，索引是fd。
*/

use std::os::unix::io::{AsRawFd, RawFd};
use std::io;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use yAsync_Runtime::*;


const HTTP_RESP: &[u8] = b"HTTP/1.1 200 OK  content-type: text/html content-length: 5  Hello";

fn main() -> Result<(),String> {
    let listener = TcpListener::bind("127.0.0.1:8000").unwrap();
    listener.set_nonblocking(true).unwrap();        /* 在没有连接到达时，调用 accept 不会阻塞程序。 */

    let listener_fd = listener.as_raw_fd();
    let epoll_fd = epoll_create().unwrap();     /* 创建一个 epoll 实例 */
    let mut key = 100;                          /* key: 用户定义的标识符，用于在事件发生时知道是哪个套接字触发的事件。 */
    epoll_ctl_add(epoll_fd, listener_fd, read_event(key))?;        /* 将文件描述符添加到 epoll 实例中，并只监听感兴趣的事件， 比如新连接到达的时候的可读事件 */

    let mut request_contexts: HashMap<u64, RequestContext> = HashMap::new();
    let mut events: Vec<libc::epoll_event> = Vec::with_capacity(1024);
    loop {
        // 这里就是告诉我们 这个描述符对应的listener发生了多少事件
        events.clear();
        let res = syscall!(epoll_wait(           /* epoll_wait 是一个阻塞系统调用，它等待 epoll 实例中注册的文件描述符上的事件发生。 */
            epoll_fd,
            events.as_mut_ptr() as *mut libc::epoll_event,  /* 将 events 向量的指针传递给 epoll_wait，允许它直接将发生的事件写入到这个预先分配的缓冲区中。 */
            1024,                                           /* 指定最多监听的事件数量，最多返回 1024 个事件。 */
            1000 as libc::c_int,                            /* 超时时间，以毫秒为单位。在这里，设置为 1000 毫秒（1 秒）。这意味着如果在 1 秒内没有任何事件发生，epoll_wait 将返回 0。 */
        )).unwrap();
        unsafe { events.set_len(res as usize) };    


        // 
        for ev in &events {
            match ev.u64 {
                100 => { /* 如果是100，表示是listener有新的可读事件（新的stream到达）；如果是其他值， 表示是在这里注册好的stream有新的可读事件 */
                    if let Ok((stream, addr))= listener.accept() {
                        stream.set_nonblocking(true).unwrap(); /* 和listener一样，设置为非阻塞然后注册 */
                        println!("new client: {}", addr);
                        key += 1;
                        epoll_ctl_add(epoll_fd, stream.as_raw_fd(), read_event(key)).unwrap();
                        request_contexts.insert(key, RequestContext::new(stream));
                    };

                    epoll_ctl_modify(epoll_fd, listener_fd, read_event(100)).unwrap();
                    /* oneshot模式：在处理完新连接后，重新注册监听器的事件，以便继续接收新连接；所有连接在触发一次过后 都需要重新绑定 */
                }
                key => { /* 如果是其他值，就是我们在上面listener注册的新的stream的有可读或者可写事件 */
                    if let Some(context) = request_contexts.get_mut(&key) { /* 如果 */
                        let events: u32 = ev.events;
                        match events {
                            v if v as i32 & libc::EPOLLIN == libc::EPOLLIN => {/* 位掩码操作，用于检查 events 是否包含 EPOLLIN 标志 */
                                context.read_cb(key, epoll_fd).unwrap();
                            }
                            v if v as i32 & libc::EPOLLOUT == libc::EPOLLOUT => {
                                context.write_cb(key, epoll_fd).unwrap();
                                // to_delete = Some(key);
                            }
                            v => println!("unexpected events: {}", v),
                        };
                    }
                }
            }
        }
    }
    Ok(())
}

fn epoll_create() -> Result<RawFd,String> { 
    let fd = syscall!(epoll_create1(0)).unwrap();
    if let Ok(flags) = syscall!(fcntl(fd, libc::F_GETFD)) {
        let _ = syscall!(fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC));
    }
    Ok(fd)
}

const READ_FLAGS: i32 = libc::EPOLLONESHOT | libc::EPOLLIN; /* EPOLLONESHOT模式：所有连接在触发一次过后 都需要重新绑定 */
const WRITE_FLAGS: i32 = libc::EPOLLONESHOT | libc::EPOLLOUT;
fn epoll_ctl_add(epoll_fd: RawFd, fd: RawFd, mut event: libc::epoll_event) -> Result<(),String> {
    syscall!(epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut event)).map_err(|x| format!("error: epoll_ctl_add:{x}"))?;
    Ok(())
}
fn epoll_ctl_modify(epoll_fd: RawFd, fd: RawFd, mut event: libc::epoll_event) -> Result<(),String> {
    syscall!(epoll_ctl(epoll_fd, libc::EPOLL_CTL_MOD, fd, &mut event)).unwrap();
    Ok(())
}
fn epoll_ctl_delete(epoll_fd: RawFd, fd: RawFd) -> Result<(),String> {
    syscall!(epoll_ctl(
        epoll_fd,
        libc::EPOLL_CTL_DEL,
        fd,
        std::ptr::null_mut()
    )).unwrap();
    Ok(())
}

fn read_event(key: u64) -> libc::epoll_event {/* 创建一个epoll_event 的信息结构体，设置好要监听的事件类型和key */
    libc::epoll_event {
        events: READ_FLAGS as u32,
        u64: key,
    }
}
fn write_event(key: u64) -> libc::epoll_event {
    libc::epoll_event {
        events: WRITE_FLAGS as u32,
        u64: key,
    }
}

#[derive(Debug)]
pub struct RequestContext {
    pub stream: TcpStream,
    pub content_length: usize,
    pub buf: Vec<u8>,
}
impl RequestContext {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buf: Vec::new(),
            content_length: 0,
        }
    }
    fn read_cb(&mut self, key: u64, epoll_fd: RawFd) -> io::Result<()> {
        let mut buf = [0u8; 4096];
        match self.stream.read(&mut buf) {
            Ok(_) => {
                if let Ok(data) = std::str::from_utf8(&buf) {
                    self.parse_and_set_content_length(data);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => {
                return Err(e);
            }
        };
        self.buf.extend_from_slice(&buf);
        if self.buf.len() >= self.content_length {
            println!("got all data: {} bytes", self.buf.len());
            epoll_ctl_modify(epoll_fd, self.stream.as_raw_fd(), write_event(key)).unwrap();
        } else {
            epoll_ctl_modify(epoll_fd, self.stream.as_raw_fd(), read_event(key)).unwrap();
        }
        Ok(())
    }

    fn parse_and_set_content_length(&mut self, data: &str) {
        if data.contains("HTTP") {
            if let Some(content_length) = data
                .lines()
                .find(|l| l.to_lowercase().starts_with("content-length: "))
            {
                if let Some(len) = content_length
                    .to_lowercase()
                    .strip_prefix("content-length: ")
                {
                    self.content_length = len.parse::<usize>().expect("content-length is valid");
                    println!("set content length: {} bytes", self.content_length);
                }
            }
        }
    }

    fn write_cb(&mut self, key: u64, epoll_fd: RawFd) -> io::Result<()> {
        match self.stream.write(HTTP_RESP) {
            Ok(_) => println!("answered from request {}", key),
            Err(e) => eprintln!("could not answer to request {}, {}", key, e),
        };
        self.stream.shutdown(std::net::Shutdown::Both)?;
        let fd = self.stream.as_raw_fd();
        epoll_ctl_delete(epoll_fd, fd).unwrap();
        syscall!(close(fd));
        Ok(())
    }
}
