# 504 duel legendre

Legendre 双实现对峙（欧拉准则 vs 平方搜索；434 的剩余版）。

- 对标：434 duel_quad → 存在性搜索锁定（`(2|7)=1=1`，非剩余返 `p-1`）
- 绕行：`a % p == 0` 先判零（499 同例）；`p-1` 表 `-1`（499 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/504_duel_legendre/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/504_duel_legendre/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/504_duel_legendre/main.sla -o /tmp/basic_504 && /tmp/basic_504
# 期望输出：1,1,6
```
