# 379 tri sigma

除数和三向对峙（全量 vs 半程 vs 逆序；368 的除数和版）。

- 对标：368 tri_cubesum → `σ(12) = 28` 三向锁定（半程版加回 `n` 本身）
- 绕行：无（逆序有符号 `int`；`n/2` 整除上界）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/379_tri_sigma/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/379_tri_sigma/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/379_tri_sigma/main.sla -o /tmp/basic_379 && /tmp/basic_379
# 期望输出：28,28,28
```
