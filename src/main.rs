use clap::Parser;
use std::path::{Path, PathBuf};
use sysinfo::{Pid, System, Signal};

// 只在 macOS 上导入 Command
#[cfg(target_os = "macos")]
use std::process::Command;

/// 协议类型
#[derive(Clone, Debug)]
enum Protocol {
    Tcp,
    Udp,
    All,
}

impl std::str::FromStr for Protocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Protocol::Tcp),
            "udp" => Ok(Protocol::Udp),
            "all" => Ok(Protocol::All),
            _ => Err(format!("无效的协议: {}，支持的协议: tcp, udp, all", s)),
        }
    }
}

/// 一个跨平台的 Unlocker 工具，用于查找并终止占用文件或端口的进程
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 根据文件路径查找占用进程
    #[arg(short, long, value_name = "FILE_PATH")]
    file: Option<PathBuf>,

    /// 根据端口号查找占用进程
    #[arg(short, long, value_name = "PORT")]
    port: Option<u16>,

    /// 只显示进程信息，不实际终止进程
    #[arg(short, long)]
    dry_run: bool,

    /// 指定协议类型 (tcp, udp, all)，默认为 tcp
    #[arg(long, default_value = "tcp", value_name = "PROTOCOL")]
    protocol: Protocol,
}

fn main() {
    let args = Args::parse();

    let pid_to_kill: Option<Pid> = if let Some(file_path) = args.file {
        find_process_by_file(&file_path)
    } else if let Some(port) = args.port {
        find_process_by_port(port, &args.protocol)
    } else {
        eprintln!("错误：请提供 --file 或 --port 参数。");
        std::process::exit(1);
    };

    if let Some(pid) = pid_to_kill {
        if args.dry_run {
            println!("找到进程: {} (PID: {})", get_process_name_simple(pid), pid);
            println!("--dry-run 模式: 不会终止进程");
        } else {
            println!("终止进程: {} (PID: {})", get_process_name_simple(pid), pid);
            kill_process(pid);
        }
    } else {
        println!("未找到相关进程");
    }
}

// 跨平台获取进程名称的简单函数
fn get_process_name_simple(pid: Pid) -> String {
    let mut s = System::new_all();
    s.refresh_processes();
    
    if let Some(process) = s.process(pid) {
        process.name().to_string()
    } else {
        "未知".to_string()
    }
}
// --- 1. 按端口查找 (跨平台) ---
fn find_process_by_port(port: u16, protocol: &Protocol) -> Option<Pid> {
    // 使用 netstat2 库，它在内部处理了跨平台差异
    let af_flags = netstat2::AddressFamilyFlags::IPV4 | netstat2::AddressFamilyFlags::IPV6;
    
    // 根据协议参数设置协议标志
    let proto_flags = match protocol {
        Protocol::Tcp => netstat2::ProtocolFlags::TCP,
        Protocol::Udp => netstat2::ProtocolFlags::UDP,
        Protocol::All => netstat2::ProtocolFlags::TCP | netstat2::ProtocolFlags::UDP,
    };
    
    let sockets_info = netstat2::get_sockets_info(af_flags, proto_flags);

    match sockets_info {
        Ok(sockets) => {
            // 优化：使用迭代器的 find_map 方法，找到第一个匹配的就返回
            sockets.into_iter().find_map(|si| {
                if si.local_port() == port {
                    // netstat2 返回的 PID 是 u32, sysinfo 需要 usize
                    si.associated_pids.first().map(|&process_id| {
                        Pid::from_u32(process_id)
                    })
                } else {
                    None
                }
            })
        }
        Err(e) => {
            eprintln!("查询网络连接失败: {}", e);
            None
        }
    }
}

// --- 2. 按文件查找 (平台特定实现) ---

// Linux 实现 (优先使用 lsof，回退到 /proc)
#[cfg(target_os = "linux")]
fn find_process_by_file(file_path: &Path) -> Option<Pid> {
    // 首先尝试使用 lsof（如果可用）
    if let Some(pid) = try_lsof_linux(file_path) {
        return Some(pid);
    }
    
    // 回退到 /proc 文件系统方法
    find_process_by_proc(file_path)
}

// Linux lsof 方法
#[cfg(target_os = "linux")]
fn try_lsof_linux(file_path: &Path) -> Option<Pid> {
    use std::process::Command;
    
    let output = Command::new("lsof")
        .arg("-t") // -t 选项只输出 PID
        .arg(file_path)
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // lsof 可能返回多个 PID，我们取第一个
                if let Some(pid_str) = stdout.lines().next() {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        return Some(Pid::from_u32(pid));
                    }
                }
            }
            None
        }
        Err(_) => {
            // lsof 命令不可用，回退到 /proc 方法
            None
        }
    }
}

// Linux /proc 文件系统方法
#[cfg(target_os = "linux")]
fn find_process_by_proc(file_path: &Path) -> Option<Pid> {
    use std::fs;

    // 获取文件的绝对路径，以便进行准确比较
    let target_path = match fs::canonicalize(file_path) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("无法获取文件绝对路径: {}", e);
            return None;
        }
    };

    if let Ok(processes) = fs::read_dir("/proc") {
        for entry in processes.filter_map(Result::ok) {
            // 检查目录名是否为数字 (PID)
            if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() {
                let fd_path = format!("/proc/{}/fd", pid);
                if let Ok(fds) = fs::read_dir(fd_path) {
                    for fd_entry in fds.filter_map(Result::ok) {
                        // 检查文件描述符是否是指向我们目标文件的符号链接
                        if let Ok(link_path) = fs::read_link(fd_entry.path()) {
                            if link_path == target_path {
                                return Some(Pid::from_u32(pid));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

// Windows 实现 (使用 Restart Manager API)
#[cfg(target_os = "windows")]
fn find_process_by_file(file_path: &Path) -> Option<Pid> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, ERROR_MORE_DATA};
    use windows_sys::Win32::System::RestartManager::{
        RmStartSession, RmRegisterResources, RmGetList,
        RM_PROCESS_INFO, CCH_RM_SESSION_KEY
    };
    use std::ptr;
    use std::mem;

    // 将路径转换为 Windows 宽字符
    let wide_path: Vec<u16> = OsStr::new(file_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    let mut session_handle: u32 = 0;

    // 1. 创建 Restart Manager 会话
    let result = unsafe {
        RmStartSession(&mut session_handle, 0, session_key.as_mut_ptr())
    };

    if result != ERROR_SUCCESS {
        eprintln!("初始化失败: {}", result);
        return None;
    }

    // 确保在函数结束时关闭会话
    let _session_guard = SessionGuard(session_handle);

    // 2. 注册文件资源
    let file_ptr = wide_path.as_ptr();
    let result = unsafe {
        RmRegisterResources(
            session_handle,
            1,              // 一个文件
            &file_ptr,      // 文件路径数组
            0,              // 无进程
            ptr::null(),    // 进程数组
            0,              // 无服务
            ptr::null(),    // 服务数组
        )
    };

    if result != ERROR_SUCCESS {
        eprintln!("注册文件失败: {}", result);
        return None;
    }

    // 3. 获取受影响的进程列表
    let mut reason: u32 = 0;
    let mut proc_info_needed: u32 = 0;
    let mut proc_info_count: u32 = 0;

    // 首先查询需要多少进程信息
    let result = unsafe {
        RmGetList(
            session_handle,
            &mut proc_info_needed,
            &mut proc_info_count,
            ptr::null_mut(),
            &mut reason,
        )
    };

    if result != ERROR_SUCCESS && result != ERROR_MORE_DATA {
        eprintln!("查询进程失败: {}", result);
        return None;
    }

    if proc_info_needed == 0 {
        // 没有进程在使用这个文件
        return None;
    }

    // 分配内存并获取进程信息
    let mut process_infos: Vec<RM_PROCESS_INFO> = vec![unsafe { mem::zeroed() }; proc_info_needed as usize];
    proc_info_count = proc_info_needed;

    let result = unsafe {
        RmGetList(
            session_handle,
            &mut proc_info_needed,
            &mut proc_info_count,
            process_infos.as_mut_ptr(),
            &mut reason,
        )
    };

    if result != ERROR_SUCCESS {
        eprintln!("获取进程信息失败: {}", result);
        return None;
    }

    // 4. 查找有效的进程并返回第一个
    for i in 0..proc_info_count as usize {
        let proc_info = &process_infos[i];
        let pid = proc_info.Process.dwProcessId;

        // 验证进程是否仍然存在且创建时间匹配
        if let Some(validated_pid) = validate_process(pid, &proc_info.Process.ProcessStartTime) {
            return Some(validated_pid);
        }
    }

    None
}

// Windows 辅助函数：验证进程是否仍然存在
#[cfg(target_os = "windows")]
fn validate_process(pid: u32, expected_start_time: &windows_sys::Win32::Foundation::FILETIME) -> Option<Pid> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, GetProcessTimes, PROCESS_QUERY_LIMITED_INFORMATION
    };
    use std::mem;

    let process_handle = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
    };

    if process_handle == 0 {
        return None; // 进程不存在或无法访问
    }

    let mut create_time: FILETIME = unsafe { mem::zeroed() };
    let mut exit_time: FILETIME = unsafe { mem::zeroed() };
    let mut kernel_time: FILETIME = unsafe { mem::zeroed() };
    let mut user_time: FILETIME = unsafe { mem::zeroed() };

    let success = unsafe {
        GetProcessTimes(
            process_handle,
            &mut create_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        )
    };

    unsafe { CloseHandle(process_handle) };

    if success == 0 {
        return None;
    }

    // 比较创建时间
    if create_time.dwLowDateTime == expected_start_time.dwLowDateTime 
        && create_time.dwHighDateTime == expected_start_time.dwHighDateTime {
        Some(Pid::from_u32(pid))
    } else {
        None
    }
}

// Windows 辅助函数：获取进程名称
#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn get_process_name(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION
    };

    let process_handle = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
    };

    if process_handle == 0 {
        return None;
    }

    let mut buffer = [0u16; 260]; // MAX_PATH
    let mut size = buffer.len() as u32;

    let success = unsafe {
        QueryFullProcessImageNameW(process_handle, 0, buffer.as_mut_ptr(), &mut size)
    };

    unsafe { CloseHandle(process_handle) };

    if success == 0 {
        return None;
    }

    // 转换为 Rust 字符串
    let path = String::from_utf16_lossy(&buffer[..size as usize]);
    
    // 提取文件名
    std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
}

// 会话守护，确保在函数结束时关闭 Restart Manager 会话
#[cfg(target_os = "windows")]
struct SessionGuard(u32);

#[cfg(target_os = "windows")]
impl Drop for SessionGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::System::RestartManager::RmEndSession;
        unsafe {
            RmEndSession(self.0);
        }
    }
}

// macOS 实现 (通过调用 lsof 命令)
#[cfg(target_os = "macos")]
fn find_process_by_file(file_path: &Path) -> Option<Pid> {
    let output = Command::new("lsof")
        .arg("-t") // -t 选项只输出 PID
        .arg(file_path)
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // lsof 可能返回多个 PID，我们取第一个
                if let Some(pid_str) = stdout.lines().next() {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        return Some(Pid::from_u32(pid));
                    }
                }
            }
            None
        }
        Err(e) => {
            eprintln!("执行 lsof 命令失败: {}", e);
            None
        }
    }
}

// 兜底实现，用于其他不支持的操作系统
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn find_process_by_file(_file_path: &Path) -> Option<Pid> {
    eprintln!("当前操作系统不支持通过文件路径查找进程。");
    None
}

// --- 3. 杀死进程 (跨平台) ---
fn kill_process(pid: Pid) {
    let mut s = System::new_all();
    s.refresh_processes(); // 刷新进程列表

    if let Some(process) = s.process(pid) {
        // 首先尝试友好地终止 (SIGTERM)
        if process.kill_with(Signal::Term).unwrap_or(false) {
            println!("进程已终止");
        } else {
            // 如果失败，强制杀死 (SIGKILL)
            if process.kill() {
                // .kill() 默认发送 SIGKILL
                println!("进程已强制终止");
            } else {
                eprintln!("无法终止进程，可能需要管理员权限");
            }
        }
    } else {
        eprintln!("进程不存在");
    }
}