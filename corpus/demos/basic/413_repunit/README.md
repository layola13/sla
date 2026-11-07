# 413 repunit

repunit 数（`R(5)=11111`；352 的 1 串版）。

- 对标：352 geom_sum → `r*10+1` 拼装（`R(2)=11` 素，`R(3)=111` 合）
- 绕行：无（纯 `int` 循环复用 `is_prime`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/413_repunit/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/413_repunit/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/413_repunit/main.sla -o /tmp/basic_413 && /tmp/basic_413
# 期望输出：11111,1,0
```
