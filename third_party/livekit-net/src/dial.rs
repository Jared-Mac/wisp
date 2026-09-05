// Wisp modification, 2026. Licensed under Apache-2.0; see NOTICE.md.
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{collections::VecDeque, future::Future, io, net::SocketAddr, time::Duration};
use tokio::net::{TcpStream, lookup_host};

const FALLBACK_DELAY: Duration = Duration::from_millis(250);

/// Bounded, staggered dual-stack TCP connection. A blackholed first address
/// must not consume the entire signaling deadline before IPv4 is attempted.
/// No spawned tasks survive cancellation, and TLS still uses the original URL.
pub async fn connect_tcp(host: &str, port: u16) -> io::Result<TcpStream> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let addresses = lookup_host((host, port)).await?.collect();
        race_addresses(interleave(addresses), |address| async move {
            tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(address))
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connection timed out"))?
        })
        .await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DNS/TCP connection timed out"))?
}

fn interleave(addresses: Vec<SocketAddr>) -> VecDeque<SocketAddr> {
    let first_is_v6 = addresses.first().is_some_and(SocketAddr::is_ipv6);
    let (mut first, mut second): (VecDeque<_>, VecDeque<_>) = addresses
        .into_iter()
        .partition(|address| address.is_ipv6() == first_is_v6);
    let mut ordered = VecDeque::new();
    while !first.is_empty() || !second.is_empty() {
        if let Some(address) = first.pop_front() {
            ordered.push_back(address);
        }
        if let Some(address) = second.pop_front() {
            ordered.push_back(address);
        }
    }
    ordered
}

async fn race_addresses<T, F, Fut>(mut addresses: VecDeque<SocketAddr>, connect: F) -> io::Result<T>
where
    F: Fn(SocketAddr) -> Fut,
    Fut: Future<Output = io::Result<T>>,
{
    let first = addresses
        .pop_front()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "DNS returned no addresses"))?;
    let mut pending = FuturesUnordered::new();
    pending.push(connect(first));
    let delay = tokio::time::sleep(FALLBACK_DELAY);
    tokio::pin!(delay);
    loop {
        tokio::select! {
            result = pending.next(), if !pending.is_empty() => {
                let error = match result.expect("pending connection exists") {
                    Ok(stream) => return Ok(stream),
                    Err(error) => error,
                };
                if let Some(address) = addresses.pop_front() {
                    pending.push(connect(address));
                    delay.as_mut().reset(tokio::time::Instant::now() + FALLBACK_DELAY);
                } else if pending.is_empty() { return Err(error); }
            }
            () = &mut delay, if !addresses.is_empty() => {
                pending.push(connect(addresses.pop_front().expect("address exists")));
                delay.as_mut().reset(tokio::time::Instant::now() + FALLBACK_DELAY);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ipv4_literal_connects_without_dns_or_ipv6() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let socket = connect_tcp("127.0.0.1", port).await.unwrap();
        assert_eq!(socket.peer_addr().unwrap().port(), port);
    }

    #[tokio::test]
    async fn unreachable_ipv6_does_not_block_ipv4() {
        let v6 = "[::1]:1234".parse::<SocketAddr>().unwrap();
        let v4 = "127.0.0.1:1234".parse::<SocketAddr>().unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            race_addresses(VecDeque::from([v6, v4]), |address| async move {
                if address.is_ipv6() {
                    std::future::pending::<()>().await;
                }
                Ok(address)
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result, v4);
    }

    #[tokio::test]
    async fn immediate_errors_advance_and_empty_dns_fails() {
        let addresses = VecDeque::from([
            "[::1]:1234".parse().unwrap(),
            "127.0.0.1:1234".parse().unwrap(),
        ]);
        let result = race_addresses(addresses, |address| async move {
            if address.is_ipv6() {
                Err(io::Error::from(io::ErrorKind::ConnectionRefused))
            } else {
                Ok(address)
            }
        })
        .await
        .unwrap();
        assert!(result.is_ipv4());
        assert!(
            race_addresses(VecDeque::new(), |_| async { Ok(()) })
                .await
                .is_err()
        );
    }

    #[test]
    fn alternate_families_without_losing_addresses() {
        let addresses = ["[::1]:1", "[::2]:1", "127.0.0.1:1", "127.0.0.2:1"]
            .map(|address| address.parse().unwrap())
            .to_vec();
        let ordered = interleave(addresses);
        assert_eq!(ordered.len(), 4);
        assert!(
            ordered[0].is_ipv6()
                && ordered[1].is_ipv4()
                && ordered[2].is_ipv6()
                && ordered[3].is_ipv4()
        );
    }
}
