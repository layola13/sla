# 381 coprime sum

互素和（`n=9 → 27`；346 的求和版）。

- 对标：346 coprime_pairs → 互素项累加（`= n·φ(n)/2`：`9·6/2`）
- 绕行：无（纯 `int` 循环复用 `gcd`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/381_coprime_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/381_coprime_sum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/381_coprime_sum/main.sla -o /tmp/basic_381 && /tmp/basic_381
# 期望输出：27,20
```
