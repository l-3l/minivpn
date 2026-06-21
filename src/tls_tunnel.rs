use anyhow::{Context, Result};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, ClientConnection, Connection, ServerConfig, ServerConnection};

use crate::tun_dev::TunDevice;

const MTU: usize = 2000;

pub fn load_server_config(cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>> {
    let certs = load_certs(cert_path)?;
    let key = load_private_key(key_path)?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("服务端 TLS 配置失败")?;
    Ok(Arc::new(config))
}

pub fn load_client_config(ca_path: &str) -> Result<Arc<ClientConfig>> {
    let mut root_store = rustls::RootCertStore::empty();
    let ca_bytes = fs::read(ca_path)?;
    let certs: Vec<CertificateDer> =
        rustls_pemfile::certs(&mut ca_bytes.as_slice()).collect::<Result<_, _>>()?;
    root_store.add_parsable_certificates(certs);
    Ok(Arc::new(ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()))
}

pub fn run_tls_server(
    tun: &mut TunDevice,
    config: Arc<ServerConfig>,
    bind_addr: &str,
) -> Result<()> {
    let listener = std::net::TcpListener::bind(bind_addr)?;
    log::info!("TLS Server 监听 {}", bind_addr);

    let (tcp, peer) = listener.accept()?;
    log::info!("接受来自 {} 的 TCP 连接", peer);
    tcp.set_nodelay(true)?;

    let tls = ServerConnection::new(config)?;
    let mut conn = Connection::Server(tls);
    do_handshake(&mut conn, &tcp)?;
    log::info!("TLS 握手完成");

    poll_loop(tun, &mut conn, &tcp)
}

pub fn run_tls_client(
    tun: &mut TunDevice,
    config: Arc<ClientConfig>,
    remote: &str,
    server_name: &str,
) -> Result<()> {
    let tcp = TcpStream::connect(remote)?;
    tcp.set_nodelay(true)?;
    log::info!("TLS Client 连接到 {}", remote);

    let dns_name = ServerName::try_from(server_name)?.to_owned();
    let tls = ClientConnection::new(config, dns_name)?;
    let mut conn = Connection::Client(tls);
    do_handshake(&mut conn, &tcp)?;
    log::info!("TLS 握手完成");

    poll_loop(tun, &mut conn, &tcp)
}

fn do_handshake(conn: &mut Connection, tcp: &TcpStream) -> Result<()> {
    loop {
        conn.write_tls(&mut &*tcp)?;
        if !conn.is_handshaking() {
            break;
        }
        if conn.read_tls(&mut &*tcp)? == 0 {
            anyhow::bail!("TLS 握手时连接断开");
        }
        conn.process_new_packets()?;
    }
    Ok(())
}

/// 单线程 poll 循环，等价于 UDP 隧道模式
fn poll_loop(tun: &mut TunDevice, conn: &mut Connection, tcp: &TcpStream) -> Result<()> {
    let tun_fd = tun.fd;
    let tcp_fd = tcp.as_raw_fd();
    let mut buf = vec![0u8; 65536];

    loop {
        let mut fds = vec![
            libc::pollfd { fd: tun_fd, events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: tcp_fd, events: libc::POLLIN, revents: 0 },
        ];

        let ret = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if ret < 0 {
            anyhow::bail!("poll 失败");
        }

        if fds[0].revents & libc::POLLIN != 0 {
            let n = tun.recv(&mut buf)?;
            log::info!("TUN→TLS: {} 字节", n);
            conn.writer().write_all(&buf[..n])?;
            conn.write_tls(&mut &*tcp)?;
        }

        if fds[1].revents & libc::POLLIN != 0 {
            if conn.read_tls(&mut &*tcp)? == 0 {
                log::warn!("TLS 连接关闭");
                break;
            }
            conn.process_new_packets()?;
            let n = conn.reader().read(&mut buf).unwrap_or(0);
            if n > 0 {
                log::info!("TLS→TUN: {} 字节", n);
                tun.send(&buf[..n])?;
            }
        }
    }
    Ok(())
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let bytes = fs::read(path)?;
    Ok(rustls_pemfile::certs(&mut bytes.as_slice()).collect::<Result<Vec<_>, _>>()?)
}

fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let bytes = fs::read(path)?;
    let mut reader = bytes.as_slice();
    loop {
        match rustls_pemfile::read_one(&mut reader)? {
            Some(rustls_pemfile::Item::Pkcs1Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Pkcs8Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Sec1Key(key)) => return Ok(key.into()),
            Some(_) => continue,
            None => anyhow::bail!("未找到私钥"),
        }
    }
}
