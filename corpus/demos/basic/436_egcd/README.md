# 436 egcd

扩展 Euclid（Bezout 恒等式；347 的系数版）。

- 对标：347 gcd_chain → 系数同步迭代（恒等式断言，避负数字面量）
- 绕行：`s0 - q*s1` 减法天然得负（无一元负号坑）；恒等式即断言

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/436_egcd/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/436_egcd/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/436_egcd/main.sla -o /tmp/basic_436 && /tmp/basic_436
# 期望输出：2,2
```
