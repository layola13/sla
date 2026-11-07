# 472 star

星形数（`S(4)=73`；466 的星形版）。

- 对标：466 pentagonal → `6n(n-1)+1` 闭式（`S(3)=37`）
- 绕行：无（纯 `int` 闭式；恒整）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/472_star/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/472_star/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/472_star/main.sla -o /tmp/basic_472 && /tmp/basic_472
# 期望输出：73,37,1
```
