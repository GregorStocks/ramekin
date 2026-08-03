//! These tests close and rebind fd 3, which is only safe when no other test
//! in the same process can open or close file descriptors concurrently. They
//! live in their own test binary (serialized by `env_lock`) for that reason;
//! don't add unrelated tests here.

use ramekin_server::{bind_listener, ListenerSource};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn bind_listener_falls_back_to_direct_bind_without_socket_activation() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::remove_var("LISTEN_FDS");
        std::env::remove_var("LISTEN_PID");
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (listener, source) = runtime.block_on(bind_listener(0));
    let addr = listener.local_addr().unwrap();

    assert_eq!(source, ListenerSource::DirectBind);
    assert!(addr.port() > 0);
}

#[cfg(unix)]
#[test]
fn bind_listener_uses_socket_activation_when_listener_is_present() {
    use std::os::fd::IntoRawFd;

    unsafe extern "C" {
        fn close(fd: i32) -> i32;
        fn dup(fd: i32) -> i32;
        fn dup2(src: i32, dst: i32) -> i32;
    }

    struct FdRestore(i32);

    impl Drop for FdRestore {
        fn drop(&mut self) {
            unsafe {
                if self.0 >= 0 {
                    assert!(dup2(self.0, 3) >= 0, "failed to restore fd 3");
                    assert_eq!(close(self.0), 0, "failed to close duplicated fd");
                } else {
                    let _ = close(3);
                }
                std::env::remove_var("LISTEN_FDS");
                std::env::remove_var("LISTEN_PID");
            }
        }
    }

    let _guard = env_lock().lock().unwrap();
    let restore = FdRestore(unsafe { dup(3) });
    unsafe {
        close(3);
    }
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let expected_addr = std_listener.local_addr().unwrap();
    let listener_fd = std_listener.into_raw_fd();

    unsafe {
        if listener_fd != 3 {
            assert!(dup2(listener_fd, 3) >= 0, "failed to set fd 3");
            assert_eq!(close(listener_fd), 0, "failed to close listener fd");
        }
        std::env::set_var("LISTEN_FDS", "1");
        std::env::remove_var("LISTEN_PID");
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (listener, source) = runtime.block_on(bind_listener(1));

    assert_eq!(source, ListenerSource::SocketActivation);
    assert_eq!(listener.local_addr().unwrap(), expected_addr);

    drop(listener);
    drop(restore);
}
