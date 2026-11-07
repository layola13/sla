# 453 duel trib

Tribonacci 双实现对峙（循环 vs 递归；432 的三阶版）。

- 对标：432 duel_peak → 三初值递归锁定（`T(10)=81=81`）
- 绕行：递归三基（`0/1` 归 0，`2` 归 1）；深度 10 安全

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/453_duel_trib/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/453_duel_trib/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/453_duel_trib/main.sla -o /tmp/basic_453 && /tmp/basic_453
# 期望输出：81,81
```
