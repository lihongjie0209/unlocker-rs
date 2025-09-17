# Unlocker-RS

一个用 Rust 编写的跨平台 "Unlocker" 工具，用于查找并终止占用文件或端口的进程。

## 版本

当前版本：v0.3.0

## 功能特性

- 🔍 **按端口查找进程**: 查找占用指定端口的进程
- 📁 **按文件查找进程**: 查找正在使用指定文件的进程
- 🌐 **协议支持**: 支持 TCP、UDP 或全部协议查找
- 🖥️ **跨平台支持**: 支持 Windows、Linux 和 macOS
- 🔍 **干运行模式**: 查看占用进程但不实际终止
- ⚡ **快速终止**: 先尝试友好终止 (SIGTERM)，如果失败则强制终止 (SIGKILL)
- 📝 **简洁日志**: 只显示关键信息，避免冗余输出

## 安装

确保你已经安装了 Rust。然后克隆并编译这个项目：

```bash
git clone <repository-url>
cd unlocker-rs
cargo build --release
```

编译后的可执行文件位于 `target/release/unlocker-rs.exe` (Windows) 或 `target/release/unlocker-rs` (Linux/macOS)。

## 使用方法

### 按端口查找并终止进程

```bash
# 查找占用端口 8080 的进程并终止（默认 TCP）
./target/release/unlocker-rs --port 8080

# 指定协议类型
./target/release/unlocker-rs --port 8080 --protocol tcp
./target/release/unlocker-rs --port 53 --protocol udp
./target/release/unlocker-rs --port 3000 --protocol all

# 干运行模式 - 只查看不终止
./target/release/unlocker-rs --port 8080 --dry-run
```

### 按文件查找并终止进程

```bash
# 查找正在使用指定文件的进程并终止
./target/release/unlocker-rs --file /path/to/your/file.txt

# Windows 示例
./target/release/unlocker-rs.exe --file "C:\Users\username\Documents\file.txt"

# 干运行模式
./target/release/unlocker-rs --file /path/to/file.txt --dry-run
```

### 查看帮助

```bash
./target/release/unlocker-rs --help
```

## 参数说明

- `-p, --port <PORT>`: 指定要查找的端口号
- `-f, --file <FILE_PATH>`: 指定要查找的文件路径
- `-d, --dry-run`: 干运行模式，只显示进程信息而不终止进程
- `--protocol <PROTOCOL>`: 指定协议类型，可选值：
  - `tcp` (默认): 只查找 TCP 连接
  - `udp`: 只查找 UDP 连接
  - `all`: 查找所有协议（TCP + UDP）

## 平台特定说明

### Windows
- 使用 Windows Restart Manager API 进行精确的文件占用检测
- 支持检测被多个进程占用的文件
- 验证进程状态以确保安全终止

### Linux
- 使用优先级策略：先尝试 `lsof` 命令（如果可用），失败时自动回退到 `/proc` 文件系统
- 需要适当的权限来访问 `/proc` 和终止进程

### macOS
- 使用 `lsof` 命令来查找占用文件的进程
- 确保系统中已安装 `lsof` (通常是预装的)

### Windows
- 端口查找功能完全支持
- **文件查找功能现已完整实现**，使用 Windows Restart Manager API 提供原生支持
- 支持查找任何被进程锁定的文件，无需外部工具

## 最新改进 (v0.3.0)

### 协议选择功能
- ✅ **协议参数**: 新增 `--protocol` 参数支持 TCP、UDP 或全部协议查找
- ✅ **默认 TCP**: 默认只查找 TCP 连接，提高查询效率
- ✅ **精确过滤**: 根据指定协议精确查找，避免不必要的扫描

### 之前版本改进 (v0.2.0)

### 用户体验提升
- ✅ **简化输出**: 大幅简化日志输出，减少冗余信息，提供更清晰的用户体验
- ✅ **Dry-run 模式**: 新增 `--dry-run` 参数，只显示进程信息而不实际终止，方便确认操作
- ✅ **Linux 优化**: Linux 下优先使用 `lsof` 命令（更快），自动回退到 `/proc` 文件系统

### Windows 文件查找功能升级
- ✅ **完整实现 Windows Restart Manager API**: 使用微软官方推荐的方式查找文件占用进程
- ✅ **进程验证**: 通过进程创建时间验证进程的有效性，避免 PID 重用问题
- ✅ **自动资源清理**: 使用 RAII 模式确保 Restart Manager 会话正确关闭

### 性能优化
- ✅ **端口查找优化**: 只查询必要的协议（TCP/UDP, IPv4/IPv6），使用迭代器提前退出
- ✅ **内存使用优化**: 减少不必要的内存分配

## 示例用法

### 场景 1: 端口被占用
```bash
# 假设你想启动一个 web 服务器在端口 3000，但收到 "地址已在使用" 错误
./target/release/unlocker-rs --port 3000
```

### 场景 2: 文件被锁定
```bash
# 假设你无法删除或修改一个文件，因为它被某个进程占用
./target/release/unlocker-rs --file "C:\logs\application.log"
```

## 安全注意事项

⚠️ **重要**: 此工具会强制终止进程，这可能导致数据丢失。请在使用前确保：

1. 你了解将被终止的进程
2. 已保存所有重要数据
3. 具有终止目标进程的必要权限

## 技术实现

- **命令行解析**: 使用 `clap` 库
- **进程管理**: 使用 `sysinfo` 库进行跨平台进程操作
- **网络查询**: 使用 `netstat2` 库进行端口查询，支持 TCP/UDP 协议
- **平台特定**: 使用条件编译 (`#[cfg(target_os = "...")]`) 处理不同操作系统
- **Windows 文件查找**: 使用 Restart Manager API (`RmStartSession`, `RmRegisterResources`, `RmGetList`)
- **Linux 文件查找**: 遍历 `/proc` 文件系统检查文件描述符
- **macOS 文件查找**: 调用系统的 `lsof` 命令

## 开发

如果你想对这个项目做出贡献或进行修改：

```bash
# 开发模式编译
cargo build

# 运行测试
cargo test

# 检查代码格式
cargo fmt

# 运行 linter
cargo clippy
```

## 许可证

MIT License - 详见 LICENSE 文件

## 贡献

欢迎提交 issue 和 pull request！

## 已知限制

1. **权限要求**: 在某些情况下可能需要管理员/root 权限来终止进程
2. **macOS 依赖**: macOS 版本依赖系统预装的 `lsof` 工具
3. **端口查询范围**: 为了兼容性，端口查询会检查所有活动连接（已针对性能进行优化）