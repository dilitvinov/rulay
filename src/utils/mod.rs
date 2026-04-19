use std::io;
use std::os::fd::AsRawFd;
use std::time::Duration;
use socket2::TcpKeepalive;
use tokio::net::TcpStream;
use std::os::unix::io::FromRawFd;

const IO_TIMEOUT : u64 = 600;
pub async fn copy_bidirectional_with_timeout(mut a: TcpStream, mut b: TcpStream) -> io::Result<(u64, u64)> {
    set_idle_timeout(&a, IO_TIMEOUT);
    set_idle_timeout(&b, IO_TIMEOUT);

    #[cfg(not(target_os = "linux"))]
    {
        tokio::io::copy_bidirectional(&mut a, &mut b).await
    }
    #[cfg(target_os = "linux")]
    {
        tokio_splice::zero_copy_bidirectional(&mut a, &mut b).await
    }
}

fn set_idle_timeout(stream: &TcpStream, idle_secs: u64) {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let sock = unsafe { socket2::Socket::from_raw_fd(fd) };

    let keepalive = TcpKeepalive::new().with_time(Duration::from_secs(idle_secs))
        .with_interval(Duration::from_secs(10)).
        with_retries(3);
    let _ = sock.set_tcp_keepalive(&keepalive);
    std::mem::forget(sock);
}