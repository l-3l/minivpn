# 错误日志 — 详细记录

> 格式说明：
> - **错误类型**: Rust 错误码 / 逻辑错误分类
> - **完整错误信息**: 编译器的原话
> - **文件位置**: 精确到行号
> - **错误代码上下文**: 出错的代码片段
> - **根因分析**: 为什么出错
> - **修复方案**: 如何修复及修复后的代码

---

## 一、编译错误

---

### 1. E0432: tun::TunBuilder 未定义

- **错误类型**: `E0432` unresolved import
- **完整错误信息**:
  ```
  error[E0432]: unresolved import `tun::TunBuilder`
   --> src/tun_dev.rs:3:5
    |
  3 | use tun::TunBuilder;
    |     ^^^^^^^^^^^^^^^ no `TunBuilder` in the root
  ```
- **文件位置**: `src/tun_dev.rs:3:5`
- **错误代码上下文**:
  ```rust
  // 第3行
  use tun::TunBuilder;  // <-- 错误在这里
  ```
- **根因分析**:
  - 最初编写代码时参照的是较旧版本的 `tun` crate 文档
  - `tun` crate v0.8 已完全移除 builder 模式 API（`TunBuilder`）
  - 新版使用 `tun::Configuration` 结构体 + `tun::create(&config)` 函数
- **修复方案**:
  - 将 `use tun::TunBuilder` 移除
  - 改用 `tun::Configuration` 的链式调用构建配置：
  ```rust
  // 修复后
  let mut config = tun::Configuration::default();
  config
      .tun_name(name)
      .mtu(1500)
      .up()
      .address(addr)
      .netmask(netmask);
  let dev = tun::create(&config)?;
  ```

- **影响范围**: `tun_dev.rs` 整个模块的 TUN 创建逻辑需全部重写

---

### 2. E0308: Device 与 Box<dyn AbstractDevice> 类型不匹配

- **错误类型**: `E0308` mismatched types
- **完整错误信息**:
  ```
  error[E0308]: mismatched types
    --> src/tun_dev.rs:24:19
     |
  24 |         Ok(Self { dev, fd })
     |                   ^^^ expected `Box<dyn AbstractDevice>`, found `Device`
     |
  help: store this in the heap by calling `Box::new`
     |
  24 |         Ok(Self { dev: Box::new(dev), fd })
     |                   ++++++++++++++   +
  ```
- **文件位置**: `src/tun_dev.rs:24:19`
- **错误代码上下文**:
  ```rust
  pub struct TunDevice {
      pub dev: Box<dyn tun::AbstractDevice>,  // 字段声明为 trait object
      pub fd: i32,
  }
  
  impl TunDevice {
      pub fn create(...) -> Result<Self> {
          let dev = tun::create(&config)?;      // tun::create() 返回 Device
          let fd = dev.as_raw_fd();
          Ok(Self { dev, fd })                  // <-- 错误：Device ≠ Box<dyn AbstractDevice>
      }
  }
  ```
- **根因分析**:
  - `tun::create()` 返回的具体类型是 `Device`（struct）
  - `TunDevice` 的字段 `dev` 声明为 `Box<dyn tun::AbstractDevice>`（trait object）
  - Rust 不会自动将具体类型 box 化
- **修复方案**: 显式调用 `Box::new()`：
  ```rust
  // 修复后
  Ok(Self { dev: Box::new(dev), fd })
  ```
（rustc 编译器直接给出了修复 hint）

---

### 3. E0404: rustls::Connection 不是 trait 而是 enum

- **错误类型**: `E0404` expected trait, found enum
- **完整错误信息**:
  ```
  error[E0404]: expected trait, found enum `Connection`
    --> src/tls_tunnel.rs:89:30
     |
  89 | fn run_forwarding_threads<S: Connection + Send + 'static>(
     |                              ^^^^^^^^^^ not a trait
  ```
- **文件位置**: `src/tls_tunnel.rs:89:30`
- **错误代码上下文**:
  ```rust
  // 错误代码：试图用 Connection 作为 trait bound
  fn run_forwarding_threads<S: Connection + Send + 'static>(
      tun: &mut TunDevice,
      session: S,
      tcp: Arc<TcpStream>,
  ) -> Result<()> {
  ```
- **根因分析**:
  - rustls 0.23 中 `Connection` 是枚举并非 trait
  - 枚举定义为：
    ```rust
    pub enum Connection {
        Client(client::ClientConnection),
        Server(server::ServerConnection),
    }
    ```
  - 枚举本身实现了 `read_tls()`, `write_tls()`, `reader()`, `writer()` 等方法（通过 match 转发）
  - `ClientConnection` 和 `ServerConnection` 均未实现共同的 trait
- **修复方案**: 改为直接接受 `rustls::Connection` 枚举：
  ```rust
  fn start_forwarding(
      tun: &mut TunDevice,
      session: rustls::Connection,   // 直接使用枚举类型
      tcp: Arc<TcpStream>,
  ) -> Result<()> {
  ```
  调用处用枚举变体构造：
  ```rust
  // Server 侧
  start_forwarding(tun, rustls::Connection::Server(tls), Arc::new(tcp))
  // Client 侧
  start_forwarding(tun, rustls::Connection::Client(tls), tcp)
  ```

---

### 4. E0425: rustls_pemfile::private_keys 函数不存在

- **错误类型**: `E0425` cannot find function
- **完整错误信息**:
  ```
  error[E0425]: cannot find function `private_keys` in crate `rustls_pemfile`
    --> src/tls_tunnel.rs:173:25
     |
  173 |         rustls_pemfile::private_keys(&mut bytes.as_slice())
      |                         ^^^^^^^^^^^^
  help: a function with a similar name exists
      |
  173 |         rustls_pemfile::private_key(&mut bytes.as_slice())
      |                         ^^^^^^^^^^^
  ```
- **文件位置**: `src/tls_tunnel.rs:173:25`
- **错误代码上下文**:
  ```rust
  fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
      let bytes = fs::read(path)?;
      let keys: Vec<PrivateKeyDer> =
          rustls_pemfile::private_keys(&mut bytes.as_slice())  // <-- 错误
              .collect::<Result<Vec<_>, _>>()?;
      keys.into_iter().next().context("未找到私钥")
  }
  ```
- **根因分析**:
  - rustls-pemfile v2.x 没有 `private_keys()` 函数
  - 取而代之的是 `read_one()` 迭代器 API，每次返回一个 PEM item
  - 需要手动循环匹配 `Pkcs1Key`, `Pkcs8Key`, `Sec1Key` 等变体
- **修复方案**:
  ```rust
  fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
      let bytes = fs::read(path)?;
      let mut reader = bytes.as_slice();
      loop {
          match rustls_pemfile::read_one(&mut reader)? {
              Some(rustls_pemfile::Item::Pkcs1Key(key)) => return Ok(key.into()),
              Some(rustls_pemfile::Item::Pkcs8Key(key)) => return Ok(key.into()),
              Some(rustls_pemfile::Item::Sec1Key(key)) => return Ok(key.into()),
              Some(_) => continue,   // 不是私钥（可能是证书），跳过
              None => anyhow::bail!("未找到私钥"),
          }
      }
  }
  ```
（根据 rustls-pemfile API 文档改写）

---

### 5. E0271: add_parsable_certificates 类型不匹配

- **错误类型**: `E0271` type mismatch
- **完整错误信息**:
  ```
  error[E0271]: type mismatch resolving `...`
    --> src/tls_tunnel.rs:32:42
     |
  32 |     root_store.add_parsable_certificates(&certs);
     |                ------------------------- ^^^^^^ expected `CertificateDer<'_>`, found `&CertificateDer<'_>`
  ```
- **文件位置**: `src/tls_tunnel.rs:32:42`
- **错误代码上下文**:
  ```rust
  root_store.add_parsable_certificates(&certs);  // <-- 传入引用
  ```
- **根因分析**:
  - `add_parsable_certificates()` 签名要求 `impl IntoIterator<Item = CertificateDer<'a>>`
  - 传入 `&Vec<CertificateDer>` 时，`IntoIterator` 的 `Item` 是 `&CertificateDer` 而非 `CertificateDer`
  - `CertificateDer` 未实现 `Copy`，不能自动解引用
- **修复方案**: 直接 move 所有权：
  ```rust
  root_store.add_parsable_certificates(certs);  // 直接传入 Vec<CertificateDer>
  ```

---

### 6. E0599: send_tls 方法不存在于 Connection 枚举

- **错误类型**: `E0599` no method named
- **完整错误信息**:
  ```
  error[E0599]: no method named `send_tls` found for struct `MutexGuard<'_, rustls::Connection>`
    --> src/tls_tunnel.rs:115:33
     |
  115 |                 let _ = session.send_tls(&mut &*tcp);
     |                                 ^^^^^^^^
  help: there is a method `read_tls` with a similar name
  ```
- **文件位置**: `src/tls_tunnel.rs:115:33`
- **错误代码上下文**:
  ```rust
  let mut session = tls.lock().unwrap();
  session.writer().write_all(&buf[..n])?;
  let _ = session.send_tls(&mut &*tcp);  // <-- 错误
  ```
- **根因分析**:
  - `ClientConnection` 和 `ServerConnection` 有 `send_tls()` 方法用于 flush 加密数据到 TCP
  - 但在 `rustls::Connection` 枚举中，该方法被命名为 `write_tls()`（与 `read_tls` 对称）
  - 这是 rustls 枚举 API 与具体类型 API 之间的命名不一致
- **修复方案**: 
  ```rust
  let _ = session.write_tls(&mut &*tcp);  // 改为 write_tls
  ```

---

## 二、实验运行时错误

---

### 7. 路由配置错误 — Server 回程路由指向错误网段

- **错误类型**: 逻辑错误（路由配置不当）
- **发生时间**: 子实验二 UDP 隧道测试
- **场景**: Server 和 Client 成功连接，UDP 隧道建立，但 ping 100% 丢包
- **错误命令**: 
  ```bash
  sudo ip netns exec s ip route add 192.168.10.0/24 dev tun0
  ```
- **错误输出**: `RTNETLINK answers: File exists`
- **根因分析**:
  - `192.168.10.0/24` 是 Server 物理网卡 `s-u` 的网段
  - 该网段已经存在于 Server 的路由表中（自动添加），试图通过 tun0 添加另一条到同一网段的路由会冲突
  - 正确的回程路由应该是 TUN 虚拟网络（`10.0.1.0/24`），而非物理网络：
    ```
    回包路径: V(192.168.20.2) → Server(s-v物理口) → 目的IP=10.0.1.2
    Server需要知道 10.0.1.0/24 走 tun0
    ```
- **修复命令**:
  ```bash
  sudo ip netns exec s ip route add 10.0.1.0/24 dev tun0
  ```
- **完整数据流向分析**:
  ```
  U(10.0.1.2) → ping → V(192.168.20.2)
  第1步: U 查路由 → 192.168.20.0/24 dev tun0 → 写入 tun0
  第2步: U的MINIVPN 从 tun0 读取, 通过 UDP 发往 Server(192.168.10.1:4433)
  第3步: S的MINIVPN 收到 UDP, 写入 S的 tun0
  第4步: S内核从 tun0 收到 IP包, 查路由 192.168.20.2 → s-v → 发送到 V
  第5步: V 收到 ICMP Echo, 回复 ICMP Reply (DST=10.0.1.2)
  第6步: V 查路由 → 默认 gw=192.168.20.1(Server) → 发送给 Server
  第7步: Server从 s-v 收到回包, 查路由 10.0.1.2 → tun0 → 写入 tun0
  第8步: S的MINIVPN 从 tun0 读取, 通过 UDP 发回 U
  第9步: U的MINIVPN 收到 UDP, 写入 U的 tun0 → ping 进程收到回包
  ```

---

### 8. iptables DROP 规则误拦截 VPN 隧道流量

- **错误类型**: 网络策略配置错误
- **发生时间**: 子实验二 UDP 隧道测试
- **场景**: UDP 隧道已建立，路由已正确配置，但 ping 依然 100% 丢包
- **排查过程**:
  - 使用 `tcpdump -i tun0` 在 U 侧抓包：0 packets captured
  - ping 包根本没有到达 TUN 接口 → 怀疑被 iptables 拦截
  - 检查 `iptables -L OUTPUT -v -n`：
    ```
    Chain OUTPUT (policy ACCEPT)
    pkts bytes target     prot opt in     out     source     destination
      15  1260 DROP       all  --  *      *       0.0.0.0/0  192.168.20.0/24
    ```
  - 15 packets matched → ping 包全部被丢弃
- **根因分析**:
  - `setup_topology.sh` 中为隔离 U 和 V 添加了规则：
    ```bash
    sudo ip netns exec u iptables -A OUTPUT -d 192.168.20.0/24 -j DROP
    ```
  - **关键误解**: 该规则作用于 OUTPUT 链，匹配所有出站接口（包括 tun0），而非仅物理接口
  - ping 包被路由到 tun0 后，仍会在 OUTPUT 链被 DROP
  - 实际上，路由表 `192.168.20.0/24 dev tun0` 已确保流量走 VPN，无需额外 iptables 隔离
- **数据流向**（错误时）:
  ```
  ping → 路由表匹配 192.168.20.0/24 dev tun0 → OUTPUT链 → DROP ✗
  ```
- **数据流向**（修复后）:
  ```
  ping → 路由表匹配 192.168.20.0/24 dev tun0 → tun0(无DROP) → MINIVPN读取 → UDP隧道 → V ✓
  ```
- **修复方案**:
  ```bash
  # 清除所有 OUTPUT 链规则
  sudo ip netns exec u iptables -F OUTPUT
  ```
  并修改 `scripts/setup_topology.sh`，移除该 DROP 规则，改为注释说明。

- **验证结果**:
  ```
  ping 192.168.20.2
  64 bytes from 192.168.20.2: icmp_seq=1 ttl=63 time=0.549 ms
  64 bytes from 192.168.20.2: icmp_seq=2 ttl=63 time=0.415 ms
  64 bytes from 192.168.20.2: icmp_seq=3 ttl=63 time=0.463 ms
  64 bytes from 192.168.20.2: icmp_seq=4 ttl=63 time=1.15 ms
  --- 4 packets transmitted, 4 received, 0% packet loss ✓
  ```
  RTT ≈ 0.4–1.15ms（UDP 隧道 + 物理网络延迟，延迟极低符合预期）

---


### 9. 子实验三：OpenSSL 生成 v1 证书导致 rustls 拒绝加载
- **错误类型**: TLS 配置错误 (UnsupportedCertVersion)
- **场景**: TLS Server 启动时 `Error: 服务端 TLS 配置失败 → invalid peer certificate: Other(OtherError(UnsupportedCertVersion))`
- **根因分析**:
  - OpenSSL 默认生成的证书版本为 v1 (`Version: 1 (0x0)`)
  - rustls 要求证书至少为 v3 (`Version: 3 (0x2)`)，因为 v3 才有基本约束等关键扩展
  - 使用 `openssl x509 -req` 时未传入 `-extfile` 参数，导致签发 v1 证书
- **修复方案**: 签发时添加 v3 扩展文件：
  ```bash
  cat > v3.ext << EOF
  basicConstraints=CA:FALSE
  keyUsage=digitalSignature,keyEncipherment
  extendedKeyUsage=serverAuth
  subjectAltName=DNS:vpn-server
  EOF
  openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -extfile v3.ext -days 365 -out server.crt
  ```

### 10. 子实验三：TLS 握手死锁 — 服务端未做握手就进入转发线程
- **错误类型**: 逻辑错误（多线程死锁）
- **场景**: TLS 隧道建立后 ping 100% 丢包，进程虽存活但不转发数据
- **根因分析**:
  - Server 端 `run_tls_server` 在 `accept()` 后直接调用 `start_forwarding()` 启动两个转发线程
  - Client 端在 `start_forwarding()` 前执行了 `do_handshake()` 阻塞式握手
  - Server 端的线程 2 (`TLS→TUN`) 在 `read_tls()` 中阻塞等待数据，但 Client 也在 `read_tls()` 中阻塞
  - 无人驱动 Server 侧的 TLS 握手状态机 → 双方都在等对方先说话 → 死锁
- **修复方案**: 在 Server 的 `run_tls_server` 中也显式执行 TLS 握手后再进入转发循环：
  ```rust
  // 修复前
  let tls = ServerConnection::new(config)?;
  start_forwarding(tun, rustls::Connection::Server(tls), Arc::new(tcp));
  
  // 修复后
  let tls = ServerConnection::new(config)?;
  let mut conn = Connection::Server(tls);
  do_handshake(&mut conn, &tcp)?;    // Server 侧也握手
  poll_loop(tun, &mut conn, &tcp);  // 进入 poll 循环
  ```

### 11. 子实验三：多线程转发中 libc::read(tun_fd) 阻塞读不工作
- **错误类型**: 系统调用行为异常
- **场景**: TLS 握手成功，两个转发线程启动，TUN 可读事件被 poll 捕获但连接失败
- **根因分析**:
  - 线程 1 使用 `libc::read(tun_fd, ...)` 阻塞读取 TUN 设备
  - 线程 2 使用 `session.read_tls()` 阻塞读取 TCP（同时持有 `Mutex<Connection>`）
  - 两个线程竞争 `Mutex<Connection>`：
    - 线程 1 持有锁写入 TLS，线程 2 无法读取 TLS 响应
    - 线程 2 持有锁读取 TLS，线程 1 无法写入 TLS
  - 且 `tun::Device` 内部的 `Reader`/`Writer` 共享 `Arc<Fd>`，但直接 `libc::read` 绕过了设备内部的状态管理
- **修复方案**: 改为**单线程 poll() 循环**（与 UDP 模式相同架构），避免锁竞争和线程同步问题：
  ```rust
  // 单线程 poll 循环
  loop {
      poll([tun_fd, tcp_fd]);
      if tun_fd 可读: tun.recv() → conn.writer() → conn.write_tls()
      if tcp_fd 可读: conn.read_tls() → conn.process() → conn.reader() → tun.send()
  }
  ```

