// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! `tpt-ruc-backend healthcheck`: asks the running server whether it is healthy.
//!
//! The container image has no `curl`, so the server checks itself. The exit
//! code tells Docker whether the container is healthy.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(3);

/// Connects to the server listening on `bind` and asks for `/healthz`.
pub fn check(bind: SocketAddr) -> bool {
    // A server bound to every interface is reached through loopback.
    let target = if bind.ip().is_unspecified() {
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), bind.port())
    } else {
        bind
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&target, TIMEOUT) else {
        return false;
    };
    if stream.set_read_timeout(Some(TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(TIMEOUT)).is_err()
    {
        return false;
    }
    let request = "GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }
    is_ok_status_line(&response)
}

/// True when a raw HTTP response starts with a 200 status line.
pub fn is_ok_status_line(response: &str) -> bool {
    response
        .lines()
        .next()
        .is_some_and(|line| line.split_whitespace().nth(1) == Some("200"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_200_status_counts_as_healthy() {
        assert!(is_ok_status_line(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}"
        ));
        assert!(!is_ok_status_line(
            "HTTP/1.1 503 Service Unavailable\r\n\r\n"
        ));
        assert!(!is_ok_status_line(""));
        assert!(!is_ok_status_line("garbage"));
    }

    #[test]
    fn nothing_listening_is_unhealthy() {
        // Port 1 is privileged and not used by this service, so nothing should answer.
        assert!(!check("127.0.0.1:1".parse().unwrap()));
    }
}
