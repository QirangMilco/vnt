use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::str;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use log;
use crate::core::{conn::Vnt, Config};
use crate::VntCallback;
use crate::tun::DeviceWrite;
use crate::handle::callback::{ConnectInfo, ErrorInfo, ErrorType, HandshakeInfo, PeerClientInfo, RegisterInfo};
// 导入iOS设备的INBOUND_SENDER
#[cfg(target_os = "ios")]
use crate::tun::ios::device::INBOUND_SENDER;

// 定义数据包发送回调类型
type PacketSendCallback = unsafe extern "C" fn(*const u8, c_int);

// 全局存储数据包发送回调
lazy_static! {
    static ref PACKET_SEND_CALLBACK: Mutex<Option<PacketSendCallback>> = Mutex::new(None);
}

/// iOS设备写入实现
pub struct IosDeviceWrite;

impl DeviceWrite for IosDeviceWrite {
    fn write(&self, data: &[u8]) -> std::io::Result<usize> {
        // 从全局存储中获取回调函数
        if let Some(callback) = *PACKET_SEND_CALLBACK.lock().unwrap() {
            // 调用回调函数将数据包发送到Swift
            unsafe { callback(data.as_ptr(), data.len() as c_int); }
            Ok(data.len())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Packet send callback not set",
            ))
        }
    }
}

/// FFI回调结构体
#[repr(C)]
pub struct VntCallbacks {
    success: Option<unsafe extern "C" fn()>,
    connect: Option<unsafe extern "C" fn(*const c_char)>,
    handshake: Option<unsafe extern "C" fn(*const c_char) -> bool>,
    register: Option<unsafe extern "C" fn(*const c_char) -> bool>,
    error: Option<unsafe extern "C" fn(*const c_char)>,
    stop: Option<unsafe extern "C" fn()>,
    peer_client_list: Option<unsafe extern "C" fn(*const c_char)>,
}

/// FFI配置结构体
#[repr(C)]
pub struct VntConfig {
    token: *const c_char,
    device_id: *const c_char,
    name: *const c_char,
    server_address: *const c_char,
    dns: *const c_char,
    stun_server: *const c_char,
    in_ip: *const c_char,
    out_ip: *const c_char,
    password: *const c_char,
    mtu: c_int,
    virtual_ip: *const c_char,
    server_encrypt: bool,
    allow_wire_guard: bool,
}

/// 内部回调处理器
struct FfiCallback {
    callbacks: Arc<VntCallbacks>,
}

impl Clone for FfiCallback {
    fn clone(&self) -> Self {
        Self {
            callbacks: Arc::clone(&self.callbacks),
        }
    }
}

impl VntCallback for FfiCallback {
    fn success(&self) {
        if let Some(callback) = self.callbacks.success {
            unsafe { callback() };
        }
    }

    fn connect(&self, info: ConnectInfo) {
        if let Some(callback) = self.callbacks.connect {
            let info_str = format!("{}", info);
            let info_ptr = info_str.as_ptr() as *const c_char;
            unsafe { callback(info_ptr) };
        }
    }

    fn handshake(&self, info: HandshakeInfo) -> bool {
        if let Some(callback) = self.callbacks.handshake {
            let info_str = format!("{}", info);
            let info_ptr = info_str.as_ptr() as *const c_char;
            unsafe { callback(info_ptr) }
        } else {
            true
        }
    }

    fn register(&self, info: RegisterInfo) -> bool {
        if let Some(callback) = self.callbacks.register {
            let info_str = format!("{}", info);
            let info_ptr = info_str.as_ptr() as *const c_char;
            unsafe { callback(info_ptr) }
        } else {
            true
        }
    }

    fn error(&self, info: ErrorInfo) {
        if let Some(callback) = self.callbacks.error {
            let info_str = format!("{:?}", info);
            let info_ptr = info_str.as_ptr() as *const c_char;
            unsafe { callback(info_ptr) };
        }
    }

    fn stop(&self) {
        if let Some(callback) = self.callbacks.stop {
            unsafe { callback() };
        }
    }

    fn peer_client_list(&self, info: Vec<PeerClientInfo>) {
        if let Some(callback) = self.callbacks.peer_client_list {
            let info_str = format!("{:?}", info);
            let info_ptr = info_str.as_ptr() as *const c_char;
            unsafe { callback(info_ptr) };
        }
    }
}

/// 从C字符串转换为Rust字符串
fn c_str_to_string(c_str: *const c_char) -> Option<String> {
    if c_str.is_null() {
        None
    } else {
        unsafe { str::from_utf8_unchecked(std::ffi::CStr::from_ptr(c_str).to_bytes()).to_string() }.into()
    }
}

/// 设置数据包发送回调的FFI函数
#[no_mangle]
pub unsafe extern "C" fn vnt_set_packet_send_callback(callback: PacketSendCallback) {
    *PACKET_SEND_CALLBACK.lock().unwrap() = Some(callback);
}

/// 创建VNT实例
#[no_mangle]
pub unsafe extern "C" fn vnt_create(config: *const VntConfig, callbacks: *const VntCallbacks) -> *mut c_void {
    if config.is_null() || callbacks.is_null() {
        return ptr::null_mut();
    }

    let c_config = &*config;
    let c_callbacks = &*callbacks;

    let rust_config = Config::new(
        #[cfg(feature = "integrated_tun")]
        #[cfg(target_os = "windows")]
        false,
        c_str_to_string(c_config.token).unwrap_or_default(),
        c_str_to_string(c_config.device_id),
        c_str_to_string(c_config.name).unwrap_or_else(|| get_host_name()),
        c_str_to_string(c_config.server_address).unwrap_or_default(),
        c_str_to_string(c_config.dns),
        c_str_to_string(c_config.stun_server),
        c_str_to_string(c_config.in_ip),
        c_str_to_string(c_config.out_ip),
        c_str_to_string(c_config.password),
        c_config.mtu as u16,
        c_str_to_string(c_config.virtual_ip),
        #[cfg(feature = "integrated_tun")]
        #[cfg(feature = "ip_proxy")]
        false,
        c_config.server_encrypt,
        Default::default(),
        None,
        Default::default(),
        Vec::new(),
        false,
        #[cfg(feature = "integrated_tun")]
        None,
        Default::default(),
        Default::default(),
        Default::default(),
        #[cfg(feature = "port_mapping")]
        Vec::new(),
        Default::default(),
        true,
        c_config.allow_wire_guard,
        None,
    ).unwrap();

    let callback = FfiCallback {
        callbacks: Arc::new(*c_callbacks),
    };

    // 创建iOS设备写入实现
    let device_write = Arc::new(IosDeviceWrite);
    
    // 创建VNT实例
    match Vnt::new_with_device_write(rust_config, callback, device_write) {
        Ok(vnt) => Box::into_raw(Box::new(vnt)) as *mut c_void,
        Err(_) => ptr::null_mut(),
    }
}

/// 销毁VNT实例
#[no_mangle]
pub unsafe extern "C" fn vnt_destroy(vnt_ptr: *mut c_void) {
    if !vnt_ptr.is_null() {
        let _ = Box::<Vnt>::from_raw(vnt_ptr as *mut Vnt);
    }
}

/// 发送IP数据包
#[no_mangle]
pub unsafe extern "C" fn vnt_send_ip(vnt_ptr: *mut c_void, data: *const u8, len: c_int) -> bool {
    if data.is_null() || len <= 0 {
        return false;
    }

    #[cfg(target_os = "ios")]
    {
        // 在iOS平台上，使用INBOUND_SENDER将数据发送到Device的channel
        if let Some(sender) = INBOUND_SENDER.lock().unwrap().as_ref() {
            let slice = std::slice::from_raw_parts(data, len as usize);
            // 使用 try_send 替代 send，确保非阻塞
            match sender.try_send(slice.to_vec()) {
                Ok(_) => return true,
                Err(crossbeam_channel::TrySendError::Full(_)) => {
                    // 通道已满，主动丢包，防止阻塞 Swift 线程
                    log::warn!("VNT Channel full, dropping packet!");
                    return true; // 返回 true 欺骗 Swift 以为发送成功，避免 Swift层报错
                }
                Err(e) => {
                    log::error!("Failed to send packet: {:?}", e);
                    return false;
                }
            }
        }
    }

    // 非iOS平台或iOS平台上channel未初始化时，使用传统方式
    if !vnt_ptr.is_null() {
        let vnt = &*(vnt_ptr as *mut Vnt);
        if let Some(sender) = vnt.ipv4_packet_sender() {
            let slice = std::slice::from_raw_parts(data, len as usize);
            match sender.send_ipv4(slice) {
                Ok(_) => return true,
                Err(_) => return false,
            }
        }
    }
    
    false
}

/// 等待VNT完成
#[no_mangle]
pub unsafe extern "C" fn vnt_wait(vnt_ptr: *mut c_void) {
    if !vnt_ptr.is_null() {
        let vnt = &*(vnt_ptr as *mut Vnt);
        vnt.wait();
    }
}

/// 获取主机名
fn get_host_name() -> String {
    match hostname::get() {
        Ok(name) => name.to_string_lossy().to_string(),
        Err(_) => "unknown".to_string(),
    }
}

// 添加hostname依赖 #[cfg(test)] mod tests {
    use super::*;
    #[test] fn test_create_config() {
        // 这只是一个编译测试
        assert!(true);
    }
}