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

    let bind_addr = format!("0.0.0.0:{}", port);
    tracing::debug!("Attempting to bind to {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", bind_addr, e));
    (listener, ListenerSource::DirectBind)
}
