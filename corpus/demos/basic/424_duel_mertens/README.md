# 424 duel mertens

Mertens 双实现对峙（直接 μ vs ω 奇偶重算；423 的求和版）。

- 对标：423 duel_sphenic → μ 双路径锁定（`M(10) = -1=-1`）
- 绕行：`0 - 1` 表负数；嵌套 `while` 内层不加 `;`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/424_duel_mertens/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/424_duel_mertens/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/424_duel_mertens/main.sla -o /tmp/basic_424 && /tmp/basic_424
# 期望输出：-1,-1
```
