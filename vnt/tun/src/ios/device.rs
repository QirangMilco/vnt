use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use crossbeam_channel::{bounded, Receiver, Sender};
use lazy_static::lazy_static;

use crate::device::IFace;

// 定义一个全局的发送器，供 FFI 调用（FFI 收到 Swift 数据后写入这里）
lazy_static! {
    pub static ref INBOUND_SENDER: Mutex<Option<Sender<Vec<u8>>>> = Mutex::new(None);
}

#[derive(Debug, Clone)]
pub struct Device {
    receiver: Receiver<Vec<u8>>, // 用于接收来自 Swift 的数据包
    buffer: Vec<u8>,             // 内部缓存，处理 read buf 小于包长的情况
}

impl Device {
    pub fn new(_name: &str, _mtu: Option<usize>) -> io::Result<Self> {
        // 创建一个有界通道，防止内存积压
        let (tx, rx) = bounded(1000);
        
        // 将发送端存入全局变量，供 vnt_send_ip 使用
        *INBOUND_SENDER.lock().unwrap() = Some(tx);

        Ok(Self {
            receiver: rx,
            buffer: Vec::new(),
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
    
    // 实现其他IFace方法的默认实现
    fn shutdown(&self) -> io::Result<()> {
        Ok(())
    }
    
    fn set_ip(&self, _address: std::net::Ipv4Addr, _mask: std::net::Ipv4Addr) -> io::Result<()> {
        Ok(())
    }
    
    fn mtu(&self) -> io::Result<u32> {
        Ok(1280) // 返回安全的MTU值
    }
    
    fn set_mtu(&self, _value: u32) -> io::Result<()> {
        Ok(())
    }
    
    fn add_route(&self, _dest: std::net::Ipv4Addr, _netmask: std::net::Ipv4Addr, _metric: u16) -> io::Result<()> {
        Ok(())
    }
    
    fn delete_route(&self, _dest: std::net::Ipv4Addr, _netmask: std::net::Ipv4Addr) -> io::Result<()> {
        Ok(())
    }
    
    fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        // 由于IFace的read方法是不可变引用，但我们需要修改内部状态，这里需要一些unsafe操作
        // 在实际使用中，Device实例应该只在一个线程中使用
        let device = unsafe { &mut *(self as *const Self as *mut Self) };
        device.read_internal(buf)
    }
    
    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        // 对于写入，我们仍然使用FFI回调的方式
        // 这部分逻辑在ffi.rs中的IosDeviceWrite实现
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "iOS TUN write should be handled by IosDeviceWrite",
        ))
    }
}

impl Device {
    fn read_internal(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // 1. 如果内部缓存有数据，先发缓存的
        if !self.buffer.is_empty() {
            let len = std::cmp::min(buf.len(), self.buffer.len());
            buf[..len].copy_from_slice(&self.buffer[..len]);
            // 移除已读取部分
            self.buffer.drain(..len);
            return Ok(len);
        }

        // 2. 阻塞等待 Swift 推送数据过来
        match self.receiver.recv() {
            Ok(packet) => {
                let len = std::cmp::min(buf.len(), packet.len());
                buf[..len].copy_from_slice(&packet[..len]);
                
                // 如果 buf 放不下整个包，剩余的存入缓存
                if len < packet.len() {
                    self.buffer.extend_from_slice(&packet[len..]);
                }
                Ok(len)
            },
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "Channel closed")),
        }
    }
}

impl Read for Device {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_internal(buf)
    }
}

impl Write for Device {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        // 写入操作应该通过IosDeviceWrite实现
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "iOS TUN write should be handled by IosDeviceWrite",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}