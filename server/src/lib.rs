use listenfd::ListenFd;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenerSource {
    DirectBind,
    SocketActivation,
}

pub async fn bind_listener(port: u16) -> (tokio::net::TcpListener, ListenerSource) {
    let mut listenfd = ListenFd::from_env();
    if let Some(listener) = listenfd
        .take_tcp_listener(0)
        .expect("failed to read externally managed listener")
    {
        listener
            .set_nonblocking(true)
            .expect("failed to make externally managed listener nonblocking");
        let listener = tokio::net::TcpListener::from_std(listener)
            .expect("failed to convert externally managed listener");
        return (listener, ListenerSource::SocketActivation);
    }

    // Listen on IPv6 too, so `localhost` clients that try ::1 first reach the
    // server. With no ::1 listener, a connection to a port inside the
    // ephemeral range (e.g. 55000) can occasionally get that same port as its
    // source and connect to itself, reading back its own request.
    match bind_dual_stack(port) {
        Ok(listener) => {
            let listener = tokio::net::TcpListener::from_std(listener)
                .expect("failed to convert dual-stack listener");
            return (listener, ListenerSource::DirectBind);
        }
        Err(e) => tracing::debug!("Dual-stack bind failed, falling back to IPv4: {}", e),
    }

    let bind_addr = format!("0.0.0.0:{}", port);
    tracing::debug!("Attempting to bind to {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", bind_addr, e));
    (listener, ListenerSource::DirectBind)
}

fn bind_dual_stack(port: u16) -> std::io::Result<std::net::TcpListener> {
    use socket2::{Domain, Protocol, Socket, Type};

    let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_only_v6(false)?;
    // Match tokio's TcpListener::bind, which sets SO_REUSEADDR on Unix.
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    let addr = std::net::SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, port));
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    Ok(socket.into())
}
