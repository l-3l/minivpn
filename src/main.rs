use anyhow::Result;
use clap::{Parser, Subcommand};

mod tun_dev;
mod udp_tunnel;
mod tls_tunnel;

#[derive(Parser)]
#[command(name = "minivpn", about = "MiniVPN - Rust VPN")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// UDP 明文隧道（子实验二）
    Udp {
        #[arg(long)]
        role: String,
        #[arg(long)]
        addr: String,
        #[arg(long)]
        tun: String,
        #[arg(long, default_value = "255.255.255.0")]
        netmask: String,
    },
    /// TLS 加密隧道（子实验三/四）
    Tls {
        #[arg(long)]
        role: String,
        #[arg(long)]
        addr: String,
        #[arg(long)]
        tun: String,
        #[arg(long, default_value = "255.255.255.0")]
        netmask: String,
        /// 服务端证书路径（server 模式）
        #[arg(long)]
        cert: Option<String>,
        /// 服务端密钥路径（server 模式）
        #[arg(long)]
        key: Option<String>,
        /// CA 证书路径（client 模式）
        #[arg(long)]
        ca: Option<String>,
        /// TLS SNI 服务器名（client 模式）
        #[arg(long, default_value = "vpn-server")]
        server_name: String,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .format_timestamp_millis()
    .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Udp { role, addr, tun, netmask } => {
            let is_server = role == "server";
            udp_tunnel::run_udp_tunnel(&addr, &tun, &netmask, is_server)?;
        }
        Commands::Tls {
            role,
            addr,
            tun,
            netmask,
            cert,
            key,
            ca,
            server_name,
        } => {
            let mut dev = tun_dev::TunDevice::create("tun0", &tun, &netmask)?;

            match role.as_str() {
                "server" => {
                    let cert = cert.expect("server 模式需要 --cert");
                    let key = key.expect("server 模式需要 --key");
                    let config = tls_tunnel::load_server_config(&cert, &key)?;
                    tls_tunnel::run_tls_server(&mut dev, config, &addr)?;
                }
                "client" => {
                    let ca = ca.expect("client 模式需要 --ca");
                    let config = tls_tunnel::load_client_config(&ca)?;
                    tls_tunnel::run_tls_client(&mut dev, config, &addr, &server_name)?;
                }
                _ => anyhow::bail!("role 必须是 server 或 client"),
            }
        }
    }

    Ok(())
}
