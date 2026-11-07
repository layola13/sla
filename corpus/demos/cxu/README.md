# CXU 模块提炼演示（demos/cxu）

把 CXU（codex-react-ui）的每个模块提炼成一个可独立运行的 SLA/SA 演示：
演示通过 = 该模块所需的 sa_std 与 SLA 能力被实证；演示撞墙 = 撞墙点即
SLA/SA 的改进项，先补地基再继续。总体路线图与排期见 sala
01_overview/08_cxu_rewrite_roadmap.html。

| 演示 | 提炼自 | 状态 |
|---|---|---|
| D1 01_bridge_stdio | 各 CLI 桥的双向 stdio 内核（spawn + JSON-RPC 行协议） | 已落地（见下） |
| D2 HTTP/WS/SSE 服务面 | server 的 hono/ws 层 | 已落地（见下） |
| D3 会话与持久化 | storage（SQLite + JSON） | 已落地（见下） |
| D4 配置面 | config（TOML + env） | 已落地（见下） |
| D5 终端 PTY | terminalPty.ts | 已落地（见下） |
| D6 文件监视 | fs.watch 用法群 | 已落地（见下） |
| D7 代码图谱 | saVmTreeSitter / saVmCodeGraph | 已落地（见下） |
| D8 认证小件 | bcrypt / keyring | 已落地（见下） |
| D9 VM 工具面 | saVmTool / JEV + 16_policy | 已落地（见下） |
| D10 静态与产物服务 | server 静态面 | 已落地（见下） |
| D11 free-key 服务 | free-key server | 已落地（见下） |
| D12 前端微缩 | web 对话屏（SAX + airlock） | 已落地（见下） |

## D1 01_bridge_stdio

- `main.sla`：用 `sa_std_process_spawn_bidi`（P0-1，sci 94c4f654）以可写
  stdin 方式 spawn `/bin/cat`，写入一行 JSON-RPC 请求，关闭 stdin 后把
  stdout 读到 EOF，校验往返长度与首尾字节，再 wait 收割退出码。
- `argv1.sa`：单参数 argv 填充宏（效 codexmini 的 X_ARGV_FILL；
  .sla 里不允许 ptr→u64 转换，故在 SA 层填 [u64; 2]）。
- 字节级回显一致性由 sci 的 C 驱动测试锁定
  （tests/sa_std_runtime.zig 的 bidi 用例）；本演示证明的是 SLA 表面。

```bash
export PATH=/content/sa_all/sci/zig-out/bin:$PATH
SA_PLUGIN_DEV=1 sa build-exe demos/cxu/01_bridge_stdio/main.sla
./main    # 输出 d1 bridge stdio ok，退出码 0
SA_PLUGIN_DEV=1 sa sla test demos/cxu/01_bridge_stdio/main.sla
```

## D2 02_http_ws_sse

- `server.sla`：v2 服务器面三件套——`GET /hello` 返回 JSON、
  `GET /events` 以 stream 响应发三条 SSE tick、`GET /ws` 升级 WebSocket
  回显一帧后关闭；服务满三请求打印 `d2 server ok` 退出。
- `client.sla`：用内置 ws_client 连 `/ws` 发文本帧并逐字节验回显。
- `byteseq.sa`：(ptr, len) 字节比较宏（指针出参式；.sla 代码生成拿不到
  普通 .sa 宏的值出参寄存器，故效 codexmini X_STR_EQ 改指针式）。
- `run.sh`：构建两端、curl 验 HTTP/SSE、client 验 WS、全绿打印
  `d2 http/ws/sse ok`。

实测踩到的契约（.sai 未注明，记此备忘）：

1. v2 WebSocket 是非阻塞 + poll 驱动：`websocket_read_v2` 无数据时返回
   状态 1（would_block），须先 `websocket_poll_v2`（readable=1）再读、
   遇 1 重试；`websocket_upgrade*` 成功后请求句柄已被实现消费，
   不可再 req_free。
2. `.sai` 里返回写作 `-> &ptr` 的 extern（如 ws_client_buffer_data），
   其结果流入 ptr 形参前需在 let 上显式标注 `: ptr` 才会强制转换。

```bash
bash demos/cxu/02_http_ws_sse/run.sh
```

## D3 03_store_session

- `main.sla`：经限定导入 `plugin/node/node_extra.sai` 用 node 插件的
  SQLite 门面（`sa_node_plugin_sqlite_*`）开库建 sessions 表，插入两条
  会话（payload 为 JSON 文本）、更新轮次，再以 query_json 取回并用
  sa_std JSON 面解析嵌套 payload（校验 model 与 temp）、做
  stringify/parse 往返，全过打印 `d3 store ok`。
- `run.sh`：构建运行后另用 python3 sqlite3 独立打开数据库核对行数、
  轮次与 payload，全绿打印 `d3 store+json ok`。
- 注意：sa_plugin_db 是自研列存引擎、不是 SQLite；SA 生态的真 SQLite
  入口就是 node 插件门面（sci 的 sa_std/plugin_sqlite.sa 宏包装之）。

本演示倒逼出的修复（均已落地）：

1. SLA 前端原先不解析 `plugin/<name>/...` 限定导入（只有 SA 阶段的
   resolver 认）——plugin_imports.zig 已补 SLA 侧解析，按
   SA_PLUGINS_HOME/installed/<name>/current/sa/ 定位接口文件，
   生成的 .sa 保留限定拼写供 SA 阶段链接。
2. http-client 插件 sap.json 广告了已内置 std 的核心 .sai，符号冒烟
   必然失败导致其与依赖者（node）永远装不上——已去广告（.sai 文件
   留仓作文档，与 http-server 先例一致）。
3. node 插件 build.zig 的 http 桥模块根指向瘦身时已删除的文件——已
   改指 sci src/runtime 的 sa_http_client.zig / sa_http_server.zig；
   两核心的共享文件改走命名导入（sci 的 "sio" 先例），避免同一文件
   落入两个模块。

```bash
bash demos/cxu/03_store_session/run.sh
```

## D4 04_config

- 前置 P0-6 已补：TOML 解析内置 sa_std（sci src/runtime/sa_toml.zig，
  13 个 sa_toml_* extern，声明在 sa_std/encoding/toml.sai）。文档句柄 +
  点分路径取值（数组可经数字段寻址），镜像 JSON 面；日期时间以原文 +
  独立 kind 保存，不静默强转；**重复键/表重定义是硬解析错误**（经
  sa_toml_last_error 报告），对齐 CXU config.toml 健康检查对重复项的
  可见性要求。sci sa-std-unit 含三组 TOML 用例（133/133）。
- `main.sla`：经 fs 面读入 config.toml（codex 风格样本：顶层标量、
  [model_providers.cxu]、[mcp_servers.jex] 的 args 数组与 env 子表、
  [[servers]] 数组表），逐项断言取值；缺失键断言显式 not-found(3)；
  环境变量 CXU_MODEL 覆盖文件 model（run.sh 注入并断言其值优先）。
- `run.sh`：另用 python3 tomllib 独立解析同一样本核对同一组值，
  全绿打印 `d4 config+env ok`。

```bash
bash demos/cxu/04_config/run.sh
```

## D5 05_terminal_pty

前置 P0-2 已落地：PTY 内置 sa_std（sci src/runtime/sa_pty.zig，Linux）——
openpty + fork + setsid/TIOCSCTTY + execvp，10 个 sa_pty_* extern
（spawn/read/write/poll/resize/winsize/pid/kill/try_wait/close，
term.sai 声明），spawn 走 16_policy 的 run 类别闸（与 process spawn 同门）。
主侧读在子进程退出后以零长读表示 EOF（Linux EIO 语义）；try_wait 状态
约定：退出码或 128+信号；exec 失败由子进程以 127 退出呈现。
sa-std-unit 136/136（含三组 PTY 用例：echo 回读、winsize/stty、127）。

演示三场景（断言在程序内，退出码分项）：① /bin/echo 输出经行规程
变 `pty-hello\r\n` 精确比对；② 初始窗口 111x33 时 `stty size` 输出
`33 111`；③ 交互 cat：resize 100x40 + winsize 读回、写 hello 读回显、
SIGKILL 后 try_wait == 137。run.sh 输出 `d5 pty ok`。

摩擦（SLA 语言面记账）：用户函数不能把 &局部数组 作为 ptr 形参传入
（capability 实参 vs plain 形参 TypeMismatch）；通行做法是结构体包缓冲
+ &Struct 引用形参（Vec/MiniWriter 先例）——本演示 PtyAcc 即此式。
ptr 形参只接受 STR_PTR/extern 返回等 plain ptr。

## D6 06_fs_watch

前置 P0-3 已落地：fs watch 内置 sa_std（sci src/runtime/sa_fs_watch.zig，
Linux inotify）——6 个 sa_fs_watch_* extern（create/add/remove/poll/
next/close，fs.sai 声明）：内部缓冲逐个递交已解析事件
（wd/mask/cookie/name），add 走 16_policy 的 fs 读闸。sa-std-unit
138/138（含两组用例：目录 create/close_write/delete 事件序列、文件
监视空名事件 + 已关句柄报错）。

演示：监视 work/ 目录后用 std fs 面写 note.txt——扫到 IN_CREATE +
IN_CLOSE_WRITE（位掩码 3）；删文件后再扫到 IN_DELETE（4）。
run.sh 全程亚秒级，输出 `d6 fs watch ok`。

## D7 07_code_graph

前置 P0-4 已落地：tree-sitter 内置 sa_std（sci src/runtime/
sa_tree_sitter.zig）——核心库与语法库一律 dlopen（核心
v0.25.10 / typescript 语法 v0.23.2，构件在 sci/artifacts/
tree_sitter/，构建脚本同目录；解析规则见 .zig 头注释：
SA_TREE_SITTER_DIR 或系统加载路径，缺核心 = available 0 +
UNSUPPORTED 显式报错，语法 ABI 越界 = UNSUPPORTED）。
15 个 sa_ts_* extern（parser/parse/tree/node 走查/field/text/
sexp），树自持源码副本供节点文本提取。sa-std-unit 140/140
（含解析 TS 抽函数名、未知语法显式报错两组）。

演示：读 sample.ts 解析后遍历根命名子节点，export_statement
解包一层，收集顶层 function_declaration 名 → 精确
`add\ngreet\n`（箭头函数的 lexical_declaration 不计，属图谱
扩展面）。run.sh 输出 `d7 code graph ok`。

## D8 08_auth

P0-6 余项已全部落地：bcrypt 与 keyring 内置 sa_std（sci
src/runtime/sa_crypto.zig，crypto.sai）。bcrypt 经 dlopen
libcrypt.so.1（crypt_gensalt_rn $2b$ + crypt_r；结果须立即拷出
——crypt_r 返回其暂存结构内部指针，连用会自比自，探针实测教训）；
keyring 经 dlopen libsecret-1.so.0（变参 *_sync + DONT_MATCH_NAME
schema，单属性 key）。sa-std-unit 142/142（bcrypt 已知答案校验 +
cost 4 往返、keyring 库可用性）。

演示：bcrypt cost 10 哈希（60 字符、$2b$10$ 前缀）+ 对错口令校验；
keyring 在 dbus-run-session + gnome-keyring-daemon 环境内做
set/get/remove 往返，缺失项显式状态 3。集合按调用选：空 = OS
默认（桌面登录 keyring）；无头环境无登录 keyring 时默认集合报
IO 错误，演示用常驻的 session 集合（内存型）——此约束已记入
crypto.sai 契约注释。

## D9 09_vm_tools

两个工具（find_definition = tree-sitter 定义查找，read_file = fs
读）只经 16_policy 的门控 open 触碰文件。演示一个二进制三种策略
场景（run.sh 驱动 SA_POLICY_JSON / SA_POLICY_TOKENS_JSON）：
A allow-list 两工具全通、二次读放行；B deny 首调即状态 10、
程序以 42 退出；C ask + 一次性令牌——每文件恰开一次，lib.ts 的
蓄意二次读因令牌已消费被拒，审计日志恰有 2 条 source=token 的
放行（已断言）。

前置改进（随 D9 落地 sci）：① 令牌此前只有宿主进程内 API
（mintToken），产物二进制 / sa run 无投递面——新增
SA_POLICY_TOKENS_JSON 环境面（严格 JSON 规格数组，bootstrap 与
interp 双路同装；无策略上下文时显式报错，绝不静默）；
② sa_ts_node_byte_range 补入（定义字节跨度）。sa-std-unit
144/144（含令牌解析/铸造/一次消费两组新用例）。

## D10 10_static_server

基于 D2 的 v2 http_server：白名单静态服务（仅 /, /index.html,
/app.js, /style.css 四个字面路径可达，其余一律 404——穿越路径
根本不到达 fs 层）；每请求经 fs 面现读文件；内容类型按文件给
（html/js/css），缓存头分层（HTML no-cache、资产
public, max-age=3600）。run.sh 用 curl 核对六个请求的状态码、
content-type、cache-control 与正文逐字节一致（含 --path-as-is
的 /../secret.txt 穿越尝试 → 404）。踩坑记账：就绪探测不能用裸
TCP 连接——会被 accept 当成一个请求消费掉；首个 curl 自带
--retry-connrefused 即可。

## D11 11_freekey_server

密钥池服务（http_server v2）：GET /key 轮换发放健康密钥
（round-robin，跳过 down），POST /report 以定长报文
"key-N fail|ok" 回报（连续两次 fail 标记 down，ok 恢复），
GET /health 给 {"up":N,"down":M}，POST /reset 清零；发放满 6
次后 /key 回 429 {"error":"rate_limited"} 直到 reset。run.sh
以 curl 驱动 12 步脚本序列，逐步核对状态码与 JSON 正文。
踩坑记账：curl --retry 会把 429 当瞬时错误重试、每次重试都是
一个新请求，会打爆服务器的定额请求预算——429 步骤须用无重试
变体。

## D12 12_frontend_mini

前端微缩（诚实范围版）：page.sax（TokenView 组件，SLA handler
记 token 数/done 态）经 sax 插件真实构建出 web 束（app.wasm +
airlock.js + index.html + app.sa）；SLA 后端在 /stream 上按
prompt 推三帧 token（Hel/lo/!）+ done 帧；client.sla 作为页面
协议的替身消费者组装出 "Hello!" 并逐帧校验。**缺口如实记账：
SAX airlock 目前没有 WebSocket 桥**——页面组件尚不能直接发起
WS，此轮的流式往返由同协议的 SLA 客户端承担；airlock WS 桥
列为 P3 前端课题。环境记账：sax 插件构建需要 LLVM ≥ 19 的
LLVM-C（LLVMDIBuilderInsertDeclareRecordAtEnd），本机装
apt.llvm.org 的 llvm-20-dev；-Dllvm=false 的 bootstrap 插件能
加载但不能产出 app.wasm。
