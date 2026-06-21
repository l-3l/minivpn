use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;

pub struct TunDevice {
    pub dev: Box<dyn tun::AbstractDevice>,
    pub fd: i32,
}

impl TunDevice {
    pub fn create(name: &str, addr: &str, netmask: &str) -> Result<Self> {
        let mut config = tun::Configuration::default();
        config
            .tun_name(name)
            .mtu(1500)
            .up()
            .address(addr)
            .netmask(netmask);

        let dev = tun::create(&config).context("创建TUN设备失败，请确认是否有root权限")?;
        let fd = dev.as_raw_fd();

        log::info!("TUN设备 {} 已创建, IP: {}/{}", name, addr, netmask);
        Ok(Self { dev: Box::new(dev), fd })
    }

    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.dev.read(buf).context("TUN recv失败")?;
        Ok(n)
    }

    pub fn send(&mut self, buf: &[u8]) -> Result<usize> {
        let n = self.dev.write(buf).context("TUN send失败")?;
        Ok(n)
    }
}
