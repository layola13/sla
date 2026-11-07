# 462 motzkin

Motzkin 数（`M(5)=21`；351 的不交弦版）。

- 对标：351 catalan → 卷积和递推（`M(4)=9`，`n≤6` 重算可承受）
- 绕行：`n=0/1` 双基归 1；循环内递归求和

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/462_motzkin/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/462_motzkin/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/462_motzkin/main.sla -o /tmp/basic_462 && /tmp/basic_462
# 期望输出：21,9,2
```
