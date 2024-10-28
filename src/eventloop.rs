use std::{
    os::unix::io::RawFd,
    collections::HashMap,
    sync::{Arc, Mutex},
};


#[derive(Debug, Clone, Copy)]
pub enum EventType {
    Readable,
    Writable,
}
impl EventType {
    fn to_flags(self) -> u32 {
        match self {
            EventType::Readable => (libc::EPOLLIN | libc::EPOLLONESHOT) as u32,
            EventType::Writable => (libc::EPOLLOUT | libc::EPOLLONESHOT) as u32,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub fd: RawFd,
    pub event_type: EventType,
}

#[derive(Clone)]
pub struct EventLoop {
    epoll_fd: RawFd,
    callbacks: Arc<Mutex<HashMap<RawFd, Box<dyn Fn() + Send>>>>,
}

impl EventLoop {
    pub fn new() -> Result<Self,String> {
        let epoll_fd = syscall!(epoll_create1(0)).unwrap();
        if let Ok(flags) = syscall!(fcntl(epoll_fd, libc::F_GETFD)) {
            let _ = syscall!(fcntl(epoll_fd, libc::F_SETFD, flags | libc::FD_CLOEXEC));
        }
        Ok(EventLoop {
            epoll_fd,
            callbacks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn register<F>(&self, event_fd: RawFd, event_type: EventType, callback: F) 
    -> Result<(),String>
    where
        F: Fn() + Send + 'static,
    {
        let mut event = libc::epoll_event {
            events: event_type.to_flags(), 
            u64: event_fd as u64,
        };

        let mut callbacks = self.callbacks.lock().unwrap();
        if let Some(_) = callbacks.get(&event_fd) {
            syscall!(
                epoll_ctl(
                    self.epoll_fd, 
                    libc::EPOLL_CTL_MOD, 
                    event_fd, 
                    &mut  event
                )
            ).map_err(|x| format!("error: epoll_ctl_mod:{x}"))?;
        } else {
            syscall!(
                epoll_ctl(
                    self.epoll_fd, 
                    libc::EPOLL_CTL_ADD, 
                    event_fd, 
                    &mut  event
                )
            ).map_err(|x| format!("error: epoll_ctl_add:{x}"))?;
            callbacks.insert(event_fd, Box::new(callback));
        }
        Ok(())
    }

    pub fn unregister(&self, event_fd: RawFd) -> Result<(),String> {
        syscall!(
            epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_DEL,
                event_fd,
                std::ptr::null_mut()
            )
        ).map_err(|x| format!("error: epoll_ctl_delete:{x}"))?;

        self.callbacks.lock().unwrap().remove(&event_fd);
        Ok(())
    }

    pub fn run(&self) -> Result<(),String> {
        let mut events = Vec::with_capacity(1024);

        loop {
            events.clear();
            let num_events = syscall!(
                epoll_wait(
                    self.epoll_fd, 
                    events.as_mut_ptr(), 
                    1024, /* epoll大小 */
                    1000 as libc::c_int, /* 超时时间 ms */
                )
            ).map_err(|x| format!("error: epoll_ctl_wait:{x}"))?;

            unsafe { events.set_len(num_events as usize) };   
            for event in &events {
                let fd = event.u64 as RawFd;
                if let Some(callback) = self.callbacks.lock().unwrap().get(&fd) {
                    callback();
                }
            }
        }
    }
}





