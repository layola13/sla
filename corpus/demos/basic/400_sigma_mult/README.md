# 400 sigma mult

sigma 乘性（`σ(12)=σ(4)σ(3)=28`；377 的乘性版）。

- 对标：377 sigma → 互素分解乘积（`σ(6)=12` 另断言）
- 绕行：无（纯 `int` 循环复用 `sigma`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/400_sigma_mult/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/400_sigma_mult/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/400_sigma_mult/main.sla -o /tmp/basic_400 && /tmp/basic_400
# 期望输出：28,28,12
```
