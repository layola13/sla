# 505 tri legendre

Legendre 三连值（`(2|11)=-1` 以 10 表之；435 的剩余版）。

- 对标：435 tri_constellation → `(2|p)` 随 `p mod 8` 翻转（`7→1`，`11→-1`，`17→1`）
- 绕行：`-1` 以 `p-1` 表之（499 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/505_tri_legendre/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/505_tri_legendre/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/505_tri_legendre/main.sla -o /tmp/basic_505 && /tmp/basic_505
# 期望输出：1,10,1
```
