use std::io::{self, Read, Write};
use std::sync::Arc;

use crate::device::IFace;

#[derive(Debug, Clone)]
pub struct Device {
    // iOS平台上，我们使用NetworkExtension框架的NEPacketTunnelProvider来管理TUN设备
    // 这里只是一个占位符，实际的设备操作会通过Swift代码调用原生API
    inner: Arc<()>,
}

impl Device {
    pub fn new(_name: &str, _mtu: Option<usize>) -> io::Result<Self> {
        // 在iOS平台上，我们不直接创建TUN设备
        // 而是通过NetworkExtension框架的NEPacketTunnelProvider提供的接口
        Ok(Self {
            inner: Arc::new(()),
        })
    }
}

impl IFace for Device {
    fn name(&self) -> io::Result<String> {
        Ok("vnt-tun".to_string())
    }

    fn version(&self) -> io::Result<String> {
        Ok("iOS NetworkExtension TUN".to_string())
    }
}

impl Read for Device {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        // 在iOS平台上，TUN数据读取由NetworkExtension框架的回调提供
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "iOS TUN read not supported directly",
        ))
    }
}

impl Write for Device {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        // 在iOS平台上，TUN数据写入由NetworkExtension框架的API提供
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "iOS TUN write not supported directly",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}