use anyhow::Result;
use std::net::UdpSocket;
use std::os::fd::AsRawFd;

use crate::tun_dev::TunDevice;

const MTU: usize = 2000;

pub fn run_udp_tunnel(
    addr: &str,
    tun_ip: &str,
    netmask: &str,
    is_server: bool,
) -> Result<()> {
    let mut tun = TunDevice::create("tun0", tun_ip, netmask)?;

    if is_server {
        run_server(&mut tun, addr)?;
    } else {
        run_client(&mut tun, addr)?;
    }
    Ok(())
}

fn run_server(tun: &mut TunDevice, addr: &str) -> Result<()> {
    let sock = UdpSocket::bind(addr)?;
    sock.set_read_timeout(None)?;
    log::info!("UDP Server 监听 {}", addr);

    let mut buf = vec![0u8; MTU];
    let mut client_addr;

    // 接收第一个包以获知客户端地址
    let (n, src) = sock.recv_from(&mut buf)?;
    log::info!("收到来自 {} 的首个报文, 长度 {} 字节", src, n);
    client_addr = Some(src);
    parse_and_log_ip(&buf[..n]);
    tun.send(&buf[..n])?;

    loop {
        let tun_fd = tun.fd;
        let sock_fd = sock.as_raw_fd();

        let mut poll_fds = vec![
            libc::pollfd {
                fd: tun_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: sock_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let ret = unsafe { libc::poll(poll_fds.as_mut_ptr(), 2, -1) };
        if ret < 0 {
            anyhow::bail!("poll 失败");
        }

        if poll_fds[0].revents & libc::POLLIN != 0 {
            let n = tun.recv(&mut buf)?;
            log::info!("TUN→网络: {} 字节", n);
            if let Some(dst) = client_addr {
                sock.send_to(&buf[..n], dst)?;
            }
        }

        if poll_fds[1].revents & libc::POLLIN != 0 {
            let (n, src) = sock.recv_from(&mut buf)?;
            client_addr = Some(src);
            log::info!("网络→TUN: {} 字节", n);
            parse_and_log_ip(&buf[..n]);
            tun.send(&buf[..n])?;
        }
    }
}

fn run_client(tun: &mut TunDevice, remote: &str) -> Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect(remote)?;
    log::info!("UDP Client 连接到 {}", remote);

    let mut buf = vec![0u8; MTU];

    loop {
        let tun_fd = tun.fd;
        let sock_fd = sock.as_raw_fd();

        let mut poll_fds = vec![
            libc::pollfd {
                fd: tun_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: sock_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let ret = unsafe { libc::poll(poll_fds.as_mut_ptr(), 2, -1) };
        if ret < 0 {
            anyhow::bail!("poll 失败");
        }

        if poll_fds[0].revents & libc::POLLIN != 0 {
            let n = tun.recv(&mut buf)?;
            log::info!("TUN→网络: {} 字节", n);
            parse_and_log_ip(&buf[..n]);
            sock.send(&buf[..n])?;
        }

        if poll_fds[1].revents & libc::POLLIN != 0 {
            let n = sock.recv(&mut buf)?;
            log::info!("网络→TUN: {} 字节", n);
            parse_and_log_ip(&buf[..n]);
            tun.send(&buf[..n])?;
        }
    }
}

fn parse_and_log_ip(data: &[u8]) {
    use etherparse::Ipv4HeaderSlice;
    if data.len() >= 20 {
        if let Ok(ip) = Ipv4HeaderSlice::from_slice(data) {
            log::info!(
                "  IP: {} → {}, proto={}, len={}",
                ip.source_addr(),
                ip.destination_addr(),
                ip.protocol(),
                data.len()
            );
        } else {
            log::info!("  IPv6或其他非IPv4报文, len={}", data.len());
        }
    } else {
        log::info!("  报文长度异常: {} 字节", data.len());
    }
}
