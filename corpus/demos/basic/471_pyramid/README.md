# 471 pyramid

方锥数（`S(5)=55`；359 的闭式版）。

- 对标：359 sq_sum → 平方和闭式（`S(4)=30`）
- 绕行：无（纯 `int` 闭式；恒整除）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/471_pyramid/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/471_pyramid/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/471_pyramid/main.sla -o /tmp/basic_471 && /tmp/basic_471
# 期望输出：55,30,1
```
