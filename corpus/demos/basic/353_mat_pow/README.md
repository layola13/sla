# 353 mat pow

斐波那契矩阵幂（`M^5 = [[8,5],[5,3]]`，打包为 `8553`；28 的幂版）。

- 对标：28 mat_mul → 单位阵连乘 5 次（标量传参，避 #1 定长数组门）
- 绕行：无（纯 `int` 循环；四元组打包断言）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/353_mat_pow/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/353_mat_pow/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/353_mat_pow/main.sla -o /tmp/basic_353 && /tmp/basic_353
# 期望输出：8553
```
