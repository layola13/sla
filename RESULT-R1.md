# rust-r1-fix RESULT — R-1（std 关联调用/宏调用在普通 fn 体内不可解析）修复终报

- 线：rust-r1-fix（depth 1，主会话派单）
- 基线：layola13/sla（Rust 仓）`cc52e91`；修复提交：本地 `43b42a7`（未推送，归主会话集成）
- 口径：普查脚本与判读逐字同形（盒 /content/rust6/census_lib.sh 的 chk_job/neg_job 原文，仅路径/HOME 不同；判读 rc=0 && 无 '^Error:' 行）

## 1. 根因（实测钉死，含一条对普查表述的订正）

**普查所说的「@test 体内可解析」是假象**：`sla check`（FrontendKind::Check）管线里 `parse_test_bodies: kind == TestCodegen`（crates/sla-cli/src/pipeline.rs），@test 体在 check 形态下根本不解析、不检查。基线探针证明：@test 体内调用虚构函数 `totally_undefined_fn()`、把字符串赋给 i32 变量，check 都 rc=0 通过。真象是：**std 可调用面在任何被检查的体内都从未装配**，而普通 fn 体是 check 唯一真查的体，所以病灶表现为「只在 fn 体红」。

缺装配三处（与 Zig 现头 753fd682 对照钉死）：

- **A. checker 的名字键 std 关联叶子整段缺失**：Zig `checkAssociatedCall` 有约 45 个名字键叶子（Vec::new/HashMap::new/future::*/poll::*/task::*/executor::*/Box/Rc/Arc::new/Atomic*::new/Cell/RefCell/Mutex/RwLock::new/File::open/VecDeque::*/mpsc::channel/thread::spawn/GrowVec::*/Chan::new/str::from_utf8 等）；Rust 的 `check_associated_call` 只有 3 个消费特例 + ptr 叶子 + 固有方法 + trait 兜底，其文件内注释自认此缺口。基线首败 600/670 文件即此（`new` 461）。
- **B. 接收者形态方法叶子级联整段缺失**：Zig 按 args[0] 剥离后类型分发 Option/Result/Atomic/Cell/RefCell/Mutex/RwLock/File/JoinHandle/Sender/Receiver/Rc/Arc/Vec/HashMap/BTree/VecDeque/Array/String/Slice/Pointer 等叶子 + push/pop/remove + iter 家族。Rust 无一。
- **C. sla-cli 从不登记导入宏**：pipeline.rs 只登记 `report.externs`；oracle 线（sla-oracle）还登记 `report.imported_macro_registry`。故全部 sa_std 宏（STR_PTR/BYTES_EQ/PTR_BYTE_ADD…）在 fn 体内 Undefined call。

**与 R-2 的同根标记（任务 4 要求）**：R-2 的 UNDEF-STD 部分（183 run）与 R-1 **同根**——test/codegen 形态下 @test 体会被真解析检查，撞同一缺失面；本批修复同时治之。R-2 的 UNDEF-MODULE 部分（前端不展开 .sla 模块正文，五期 §12.8 残留）是**另一根因**，不在本线范围：`Duration::from_millis`（7 文件）、`Instant::now`/`SystemTime::now`（6）、`redis_push_lit`（14）、`sla_*`（~15）等文件名集属该族，本批未翻、未冒领。

## 2. 修法（11 文件，+2301/−22；发射路径不消费 HashMap 迭代序；全部文件 ≤700 行）

- `crates/sla-checker/src/check_leaves.rs`（新，652 行）：共享提取器/构造器 + `is_copy_value` + Zig 浅拷贝实参口径移植 + `check_std_assoc_leaf`（A 的全部叶子，位置与 Zig 相同：ptr 叶子后、固有方法前）；future::*/poll::* 委托给既有 preassoc 裸形体（门判等价）。
- `crates/sla-checker/src/check_receiver_leaves.rs` + `check_receiver_leaves2.rs`（新，694+555 行）：B 的接收者级联 `check_receiver_leaf_call`，接入点在 check_call 中紧邻 `check_receiver_call` 之前。含 Option/Result 叶子（新增 `check_closure_arg` 上下文填充闭包参数）、Atomic 叶子（Ordering 门）、Cell 集合（Zig 原文报错）、map/set 叶子（推断具化 + insert 消费纪律）、push/push_back 消费纪律（含 LoopConditionalConsume 门与 UncloneableCopy 响亮报错）、Rc/Arc clone/计数、DEF-deep-copy 克隆面、as_ptr、指针 `add`。`str::from_utf8` 从 check_call 尾部调用（Zig 的迟位置）。
- `crates/sla-checker/src/check_expr.rs`（+16）：`Ordering::Variant` 标识符臂（解析器把 `Ordering::SeqCst` 编成单个标识符；五变体给 Ordering 型，未知变体落回 UndefinedVariable）。
- `crates/sla-checker/src/check_call.rs`：两处接线 + str 尾 + GrowVec 的 sum 阶段。
- `crates/sla-cli/src/pipeline.rs`（+19）：C 的导入宏登记环，与 sla-oracle 逐字同形。
- `crates/sla-checker/src/tests_r1.rs`（新，276 行）：**26 条回归测试**（见 §4）。
- `docs/divergences/checker.md`：R-1 收口节 + 残留登记。

复跑中发现的移植自伤一例（已修、已入测试）：指针 `add` 首版对 `peel_borrow_pointer` 结果匹配 Pointer——剥离本身吃掉 Pointer 层，叶子永不触发（rosetta/112 红）；改为对未剥离接收者匹配（Zig 原口径 `recv_ty.* == .pointer`）。

## 3. 同口径复跑名集差分（盒隔离 HOME plhome-rust6r1，release 构建；两轮，第二轮含 add 修复）

| 口径 | 基线 | 修复后 | 差分 |
|---|---|---|---|
| check 级全集（2022 文件） | 绿 1045 / 红 977 | **绿 1620 / 红 402** | **翻绿 575，翻红 0** |
| 其中 tests 文件（747） | 绿 281 | 绿 421 | +140 |
| 其中 demos 文件（1275） | 绿 764 | 绿 1199 | +435 |
| demos 单元（入口红=单元红，1258） | 红 496 | **红 65** | −431 |
| R-1 首败名文件（基线 sig 665） | — | 翻绿 563 | 仍红 102（见下） |

翻绿按基线首败名 Top：`new` ×461、STR_PTR ×10、push ×8、defer_ready ×8、ready ×7、PTR_NULL ×6、unwrap_or ×5、BYTES_EQ ×5、iter ×4、unwrap/spawn/into_iter/PTR_BYTE_AT/PTR_BYTE_ADD/JSON_PARSE ×3、strong_count/open/map/is_ok/from_raw/fill ×2、send/read/lock/from/channel/borrow_mut/as_ptr ×1。

仍红 402 的新首败构成：316 为 OTHER（多为 TypeMismatch 深层——解析已通、类型层挡住，属 R-5 领地）、导入名族 ~46（redis_push_lit 14、from_millis 7、now 6、sla_* ~15，属 .sla 展开族）、UseAfterMove 13。R-1 首败仍红的 102 文件里，66 个新首败已是 TypeMismatch（即解析层清、深层未清），5 个是 chan 接收者叶子（见残留②）。

## 4. 回归测试与红前验证

- workspace 全量：基线 500/0 → **526 通过 / 0 失败**（只增不减）；盒上 sla-checker 单测 200/0。
- 26 条新测试钉住最小复现（Vec::new+push、HashMap、future、Atomic+Ordering、导入宏、负例门：Vec::new 元数、push 元素类型、fetch_add 非 Ordering 实参、`Ordering::Bogus`、unsafe 外 Box::from_raw、HashMap insert 值类型、指针 add 等）。
- 红前实证：整批回退后 18/19 原正例失败（导入宏的 checker 级测试钉的是既有机器、两向皆过；修复 C 的红前在二进制级实证：基线二进制对真实 sa_std 导入的 STR_PTR 探针报 `Undefined call: STR_PTR` rc=1，修复后 rc=0）。

## 5. 负例口径变化（131 → 129，两条均已定性，非本批缺陷）

- `tests/test_error_growvec_ref_escape.sla`、`tests/test_error_loop_consume_fill.sla`：基线「拒绝」文本字面即 `Undefined call: new`——是**虚假通过**（解析层先死、真守卫从未被测）。修复后探针证明两条真守卫在 Rust checker 里对**任何**容器都未移植（元素借用逃逸守卫、结构体字面量填充消费纪律），属 **R-6 族缺失守卫**，已登记待 R-6 批；本批未越界补（结构体填充纪律涉 DEF-deep-copy 合成辖区：Rc 字段 bump/Map-Set 响亮拒绝等成套规则）。

## 6. 残留登记（写入 docs/divergences/checker.md，六项）

① Chan::new 的标注驱动 pending 表未移植（简化为元数+数值上限→Chan<Infer>）；② chan 接收者叶子（send/recv/close/destroy/len/cap，与 Zig 的 chan_calls 表焊接）未移植——5 文件仍红，名单在 §3；③ 可迭代臂错误种类与 Zig 分歧（非可迭代接收者落回后续解析而非 TypeMismatch）；④ arena move-arg 路由未移植；⑤ insert 消费守卫为近似（仅借用实参走 Zig 精确守卫）；⑥ 结构体克隆缺 Zig 末段浅拷贝门；另 `__slice_view` 未移植。

## 7. 读数（供主会话并列汇报）

- Rust 版 check 级口径：**1045/2022（51.7%）→ 1620/2022（80.1%）**；demos check 首败单元 496 → 65。
- 普查 1976 分母口径（双后端全绿+负例响亮拒绝）本批**不动**——本批只到 check 级，run/test 级翻转须待后续批次整普查复跑后统一记账，勿把 check 级数字冒充 1976 读数。
