use std::io;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_io_timeout::TimeoutStream;

const IO_TIMEOUT : u64 = 600;
pub async fn copy_bidirectional_with_timeout<A, B>(a: &mut A, b: &mut B) -> io::Result<(u64, u64)>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let mut with_timeout_a = TimeoutStream::new(a);
    with_timeout_a.set_read_timeout(Some(Duration::from_secs(IO_TIMEOUT)));
    with_timeout_a.set_write_timeout(Some(Duration::from_secs(IO_TIMEOUT)));
    let mut a = Box::pin(with_timeout_a);
    let mut with_timeout_b = TimeoutStream::new(b);
    with_timeout_b.set_read_timeout(Some(Duration::from_secs(IO_TIMEOUT)));
    with_timeout_b.set_write_timeout(Some(Duration::from_secs(IO_TIMEOUT)));
    let mut b = Box::pin(with_timeout_b);
    tokio::io::copy_bidirectional(&mut a, &mut b).await
}