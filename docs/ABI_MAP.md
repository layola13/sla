# ABI_MAP — 宿主(sci) ↔ SLA 插件 的 C ABI 全面

参照源（本机只读）：
- 插件侧：`~/workspace/local-sa/sa_plugin_sla/src/plugin_api.zig`（119 行）、
  `plugin.zig`（271 行）、`plugin_handler.zig`（169 行）、`plugin_skills.zig`、
  `plugin_commands.zig`（617 行）、`plugin_imports.zig`。
- 宿主侧：`~/workspace/local-sa/sci/src/plugins.zig`（5336 行）。

Rust cdylib（crates/sla-plugin）须实现同一描述符 ABI；字段顺序/宽度按下表
逐项镜像。**注意**：其中 `PluginDescriptor`/`HostStream`/handler 结构体是
`extern struct`（真 C 布局）；而 `Context` 在两侧都是普通 Zig struct、
靠「同一份源码定义 + 同一编译器」保证布局一致——这是约定而非 C ABI
（见 DIVERGENCES D-C1）。

## 1. 导出符号（libsla.so 实测 nm -D）

| 符号 | 类型 | 说明 |
|---|---|---|
| `saasm_plugin_descriptor_v1` | 数据 (D) | 静态 `PluginDescriptor` 实例，宿主首选 |
| `saasm_plugin_descriptor_v1_fn` | 函数 (T) | `fn(out: *PluginDescriptor) void`，宿主次选 |
| `sla_compile_handler` | 函数 (T) | 第二 ABI：状态处理函数编译（见 §5） |
| `sla_compile_handler_result_free` | 函数 (T) | 第二 ABI 的结果释放 |

> **更正（2026-11-07，三期 HND 评估 + 协调线复核）**：上表后两行
> 在现头（sla `6002b386`）**不成立**——场地三手法互证（已装与
> zig-out 两份 libsla.so 的 nm -D 均只有描述符双锚点、strings
> 计数 0、dlopen 查无）+ 协调线直查 zig-out 复核：出货插件未
> 导出第二 ABI，sci 宿主内亦无该符号的任何消费者。本表写作时
> 参照的本地树为 `3de91932`（2026-10-26），成因（曾导出后丢失 /
> 当时测量有误 / 构建形态差异）未定，已落反馈单 **FB-035** 候
> 主线裁定（补导出 vs 退役该面）；§5 的接口形态描述保留作裁定
> 材料，不再作为现头事实引用。

宿主加载顺序（`loadDescriptor`）：先查数据符号，取其副本；无则查函数
符号并调用填充；两者皆无 → 诊断 "missing descriptor"，插件被拒。

## 2. PluginDescriptor（extern struct，64 字节，align 8）

| 偏移 | 字段 | 类型 | SLA 的取值/语义 |
|---|---|---|---|
| 0 | abi_version | u32 | 恒 1（`plugin_api.abi_version`） |
| 4 | descriptor_size | u32 | `@sizeOf(PluginDescriptor)` = 64 |
| 8 | name | [*:0]const u8 | `"sla"`（NUL 结尾静态串） |
| 16 | init | ?fn(*const Context) -> u32 | null |
| 24 | prebuild | ?fn(*const Context, ?*anyopaque) -> u32 | null |
| 32 | postbuild | ?fn(*const Context) -> u32 | null |
| 40 | handle_command | ?fn(...) -> u32 | `runSlaCommandAbi`（见 §3） |
| 48 | skills_ptr | [*]const SkillSection | 静态数组首址（1 节，见 §4） |
| 56 | skills_len | usize | 1 |

校验（宿主 `validateDescriptor`）：abi_version ≠ 1 → "abi version mismatch"；
descriptor_size ≠ 宿主侧 @sizeOf → "descriptor size mismatch"；
name 为空 → "empty plugin name"。任一不过插件被拒并记诊断。
校验通过后宿主立即调用 init（若非 null），init 返回非 ok 则拒。

所有权：描述符及其 name/skills 指向的都是插件 .so 内的静态数据，
宿主只读借用，生命周期 = .so 加载期；宿主侧 LoadedPlugin 持有
DynLib 句柄与描述符副本，卸载时先 close 库。

## 3. handle_command 与 Context

签名：
```
handle_command(ctx: *const Context,
               argv: [*]const [*:0]const u8, argv_len: usize,
               stdout: HostStream, stderr: HostStream,
               out_code: *u8) -> u32  // AbiStatus
```

- argv 约定：argv[0] = 宿主可执行名（"sa"），argv[1] = `"sla"` 或别名
  `"slab"`，argv[2] = 子命令，其后为该命令参数（均为 NUL 结尾串，
  宿主以 dupeZArgs 复制、调用后释放；插件只在调用期内借用）。
  args.len < 2 或 argv[1] 非 sla/slab → 实现返回 null → 宿主记
  `unknown_command`。
- out_code：插件写进程式退出码（u8）；返回 AbiStatus.ok 时宿主采用它。
- AbiStatus：0 ok / 1 unknown_command / 2 failed / 3 version_mismatch /
  4 invalid_descriptor。宿主把 failed/version_mismatch/invalid_descriptor
  打成诊断输出。
- 插件侧实现要点（runSlaCommandAbi）：用 page_allocator 把 ctx 拷成
  可变副本并塞入 allocator、把 argv 转成切片、把 HostStream 包成
  Writer 后调 runSlaCommandImpl；异常统一映射 failed。

Context（宿主构造、指针传入，两侧同定义镜像，约 88 字节）：

| 字段 | 类型 | 说明 |
|---|---|---|
| allocator | Zig Allocator（ptr+vtable，16B） | 宿主分配器；插件命令实现实际多用 page/arena 自备 |
| host_version | ?[]const u8 | 如 "sci-0.1" |
| log | ?fn(*const anyopaque, LogLevel u8, [*]const u8, usize) | 宿主日志回调，可空 |
| log_ctx | ?*anyopaque | 日志上下文 |
| json_mode | bool | `sa --json` 类调用时为真（skills/stability 输出 JSON） |
| broker_abi_version | u32 | 尾部追加字段（broker 扩展） |
| broker_call | ?fn(?*anyopaque, op u32, ?*const anyopaque, ?*anyopaque) -> u32 | 宿主中介回调 |
| broker_ctx | ?*anyopaque | 中介上下文 |

Broker 约定：op 5 = cache_lookup、6 = cache_store，请求/响应为
`BrokerCacheLookup{Request,Response}` / `BrokerCacheStore{Request,Response}`
extern 结构体（ptr+len 切片对 + 命中/长度字）。插件编译缓存经此向宿主
存取产物字节；broker 不可用（call 为 null 或 abi_version < 1）时插件
必须走无缓存路径。LogLevel：0 debug / 1 info / 2 warn / 3 err。

HostStream（按值传，16B）：{ ctx: ?*anyopaque, write_all: ?fn(ctx,
bytes ptr, len) -> u32 }。write_all 返回 AbiStatus，插件写失败即中止。

## 4. SkillSection（描述符 skills 数组元素，48B）

{ name: []const u8, summary: []const u8, items: []const []const u8 } —
三组 ptr+len 切片（items 是切片的切片）。SLA 只有一节：
name="sla"，summary="Sla compiler and tools"，items 为 14 条命令用法串
（init / skills / stability / build / build-workspace / build-exe /
build-wasm / build-js / build-c / sab build / sab workspace /
slab build|workspace|disasm / check / test）。宿主用它生成
`sa sla skills` 输出与 agent SKILL.md。

## 5. 第二 ABI：sla_compile_handler（状态处理函数编译）

宿主在编译 SAX/宿主侧状态处理函数时，按符号从已加载的 sla 插件取用。
签名：
```
sla_compile_handler(handler_name_ptr, handler_name_len,
                    handler_source_ptr, handler_source_len,
                    fields_ptr, fields_len,          // SlaHandlerStateFieldAbi[40B/个]
                    options_ptr,                     // ?*SlaCompileHandlerOptionsAbi[16B]
                    out: ?*SlaCompileHandlerResultAbi) -> u32
sla_compile_handler_result_free(result: ?*SlaCompileHandlerResultAbi) -> void
```
- SlaHandlerStateFieldAbi：{ name ptr+len, ty: u32, address ptr+len }；
  ty 码：1=i1, 2=i32, 3=i64, 4=f64, 5=ptr，未知码 → InvalidStateFieldType。
- SlaCompileHandlerOptionsAbi：{ base_dir ptr+len }，options_ptr 可为 null。
- SlaCompileHandlerResultAbi（48B）：{ body ptr+len, support ptr+len,
  error_name ptr+len }。
- 返回码：0 成功（body/support 有效）；1 参数无效（error_name 指出哪项，
  如 InvalidHandlerName）；2 编译失败（error_name = Zig 错误名）。
- **内存约定**：body/support 由插件内 c_allocator 分配，所有权转移给
  宿主；宿主用完必须回调 result_free，由插件以同一 c_allocator 释放
  两段并把结果结构体清零。error_name 指向静态串，不释放。

## 6. 宿主侧加载与命令分发路径

1. 插件发现：已安装插件目录（`SA_PLUGINS_HOME`，或
   `$XDG_DATA_HOME/sa_plugins`，或 `~/.local/share/sa_plugins` 下
   `installed/<name>/current/`）+ 工作区 dev 插件（sa.mod
   `require_plugin` 本地目录 → `.sa-cache/workspace-plugins`，
   源码戳判新鲜；`SA_PLUGIN_DEV=1` / `--dev` 开 dev 模式）。
2. DynLib.open → loadDescriptor → validateDescriptor → init。
3. 命令认领：manifest 的 commands 索引（认领者直达）优先；否则按
   descriptor.name 与命令名匹配/逐个 offer。kind=language 的插件另经
   「扩展名 → 前端」注册表分发（.sla 文件由宿主主动词 build/check/test
   直达前端，`sa sla ...` 形式已标 deprecated 但保留）。
4. 宿主构造 Context（allocator、log、json_mode、broker）与
   HostStream（包 stdout/stderr Writer），dupeZArgs 后调用
   handle_command，按 AbiStatus 与 out_code 收尾。
5. 权限面：LoadedPlugin 带 RuntimePermissionPolicy（能力门控），
   与本 ABI 正交，Rust 侧一期不实现策略、只实现被调用面。

## 7. 命令面（runSlaCommandImpl 分发表）

`help`（用法打到 stderr，rc 0）、`init [path]`、`skills [--json]`、
`stability schema|verify ...`、`build [file] [-p pkg] [--out file]`
（文本 SA 产物，compileSlaToSaString：读源 → source_expand →
import 展开 → 单态化 → 类型检查 → lowering → 文本 codegen）、
`build-workspace`、`build-exe`、`build-wasm`、`build-js`、`build-c`
（均为 SAB 编译后转调宿主主动词）、
`sab build|workspace|disasm`（`slab` 别名同达）、
`check [file] [-p pkg]`（parse+检查不产码）、
`test [file] [-p pkg] [--test-backend auto|sab|sa]`（编译测试输入后
转调 `sa test`）。未知命令 → stderr "Unknown sla command" + rc 1。
