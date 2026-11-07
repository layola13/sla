# 分歧登记 — oracle 线二期（全管线对拍，2026-11-07 场地三）

对拍口径：Rust 管线（sla-oracle，含 INT-1 修复后 checker）vs 场地真
Zig 插件 `sa sla build` 产物（sa 0.1.8，SA_STD_DIR=/content/sa_all/sci/
sa_std）。证据在场地树与本机主本 `results/p2/`（逐探针 run1/run2 转储、
Zig 产物、rc、stderr）。基线结果：e2e_c 逐字节全等；e2e_a / e2e_b 各有
分歧，定位如下。本文件两条均为**新发现**，未自裁为有意分歧。

## D-O1 （c）return 出口清理名单顺序：Zig `!b !a !c !d` vs Rust `!a !b !c !d`

- **程序**：`docs/probes/e2e_a.sla` 的 `arith_values`（局部绑定声明序
  a、b、c、d，return 处统一释放）。
- **现象**：最终文本在 return 前清理段顺序不同——Zig 为
  `!tmp_23 / !b / !a / !c / !d / return`，Rust 为
  `!tmp_23 / !a / !b / !c / !d / return`。其余指令逐字节相同
  （diff 全文仅此一处 + D-O2 的 @test 段）。
- **最早分歧段**：oracle 转储不暴露清理名单（03_checked 只转储
  structs/enums 名单），故**可见**最早分歧在文本发射段（06_text）；
  名单本身在 checker 侧表生成（CheckInfo.cleanups），根因候选在
  checker 段。
- **根因候选**：已在册 **checker D-C6**——Zig 块退出清理名单顺序 =
  StringHashMap 哈希迭代序（顺序敏感性当时未证实）。本条是其首个
  字节级实证：四绑定中 b 排到 a 之前，与声明序不符、与哈希序相容；
  对照 e2e_b 的 `while_sum`（t、i 两绑定）两边顺序恰同、逐字节无差，
  说明分歧只在哈希序 ≠ 声明序时显形。
- **语义**：本例四者皆 i32（Copy），`!x` 为寄存器释放，顺序不影响
  结果；但非 Copy 类型的 drop 顺序可观察，口径必须定案。
- **分类**：**Zig 侧缺陷候选**（→ FB-020）。Rust 侧按声明序发射是
  确定性选择；全线纪律「发射路径禁消费 HashMap 迭代顺序」禁止 Rust
  反向迁就 Zig 的哈希序。若主线裁定 Zig 顺序即规约，本条改判移植
  缺陷并另议确定性等价序——oracle 线不自裁。

## D-O2 （a，本移植缺陷 — oracle 线自身）@test 体裁剪未接线 【已销账 2026-11-07】

- **现象**：Zig `sa sla build`（build 模式）在 codegen 前清空 @test
  体（产物中 @test 只剩空壳 `return`）；Rust oracle 未做此裁剪，
  发射完整 test 体。e2e_b 的全部字节差即此一段（diff 单 hunk，
  纯增 18 行）；e2e_a 除 D-O1 外亦含此段。
- **在册依据**：D-T7 / FB-008 已定层——清空动作属流水线（oracle）
  职责，对拍台须以 build 模式口径喂入。oracle 一期未接此职责，
  本条即该缺口的对拍显形。
- **分类**：**移植缺陷（oracle 线）**。修复方向（候主会话排期）：
  oracle 在 parse 后、check 前按 build 模式清空 TestDecl 体，
  或增 `--build-mode` 开关与之对齐；check 段转储口径随之注明。
  本轮只登记，不顺手改（先登记再分类纪律）。

### 销账（D-O2 收尾小线，2026-11-07 场地三复跑）

- **清空语义（定准）**：整段体清空、保留声明壳。AST 层 =
  `TestDecl { name, is_ignored, should_panic, body: [] }`：语句表
  整体置空，声明本身不删除、名字与修饰符不动；AST 里不塞合成
  return——产物中的 `return` 是 codegen `gen_test_decl` 对空体
  恒加的收尾。发射形态 = `@test "name"():` + `L_TEST_ENTRY_N:` +
  `return`。依据三处一致：① Zig 本窗产物（e2e_b 空壳原文）；
  ② fixtures/expected 五件 golden 的 @test 段同形；③ 一期 text
  线对拍台以空体手搭 AST 喂入（`fixture_parity.rs` 的
  `test_decl()` 注释明示 build 模式喂入形态）。
- **实现点**：`crates/sla-oracle/src/main.rs`——parse 之后、check
  之前，`prune_test_bodies` 对顶层每个 TestDecl 原位 `body.clear()`
  （不 clone/不重建树，存活节点地址不动，守 D-T10 地址键纪律）。
  build 模式为**默认**（compare/bench 不带开关即走 build 口径）；
  `--no-build-mode`（别名 `--keep-test-bodies`）保留全体作诊断，
  `--build-mode` 显式开启。02_ast 转储为裁剪后树（后续阶段的实际
  输入）；03_checked 转储首行注明 `mode: build (@test bodies
  pruned before check: N)`。单元测试 2 件守此函数（壳保留/幂等/
  无 test 树不变）。
- **复跑证据**（`results/p2/d_o2/`，真插件本窗双跑 + Rust 双跑）：
  - e2e_b：**逐字节全等** — Rust run1/run2 与 Zig zigwork/zigwork2
    四方同 sha `8623e8d0b1326cdd…`，diff 为空。
  - e2e_c：**维持全等** — 四方同 sha
    `fe1902818c4a175e…`（与基线 sha 一致，Zig 本窗双跑互等，
    无窗差）。
  - e2e_a：**只剩 D-O1** — diff 全文仅 `!b` 一行位置
    （`!b !a !c !d` vs `!a !b !c !d` 清理序），@test 段差消失；
    按纪律未迁就 Zig 发射序，剩它即正确。
  - 确定性自检：三探针 run1/run2 sha 逐一相等（diff rc=0）。
  - 对照：`--no-build-mode` 产物与修复前基线产物逐字节同 sha
    （e2e_a `be77562f…`、e2e_b `317a69ca…`），证明开关忠实保留旧
    行为、默认口径为唯一变量。
  - 测试数字：`cargo test -p sla-oracle` 2/2（新增 2 件）；
    workspace 总数 136（基线 134 + 2，未减）；check_file_lines
    100 文件全 ≤700（main.rs 535 行）。compare.sh 仍 0/5
    FAIL(run)，与基线日志同形——系 INT-2 既有限制（fixtures 含
    std 面调用、check 段 rc=5），非本线回归。
