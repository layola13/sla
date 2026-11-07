# 346 coprime pairs

互素对判定（`gcd == 1` 即互素；336 计数版的判定版）。

- 对标：336 欧拉函数（计数）→ 本例成对判定
- 绕行：无（纯 `int` 取模循环；`is_coprime` 以 `1/0` 代替布尔返回）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/346_coprime_pairs/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/346_coprime_pairs/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/346_coprime_pairs/main.sla -o /tmp/basic_346 && /tmp/basic_346
# 期望输出：1,0,1
```
