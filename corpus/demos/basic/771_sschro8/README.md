# 771 sschro8

小 Schröder 高位二 `s(8)=20793`（`s(7)=4279` 低位锚；短递推一步 `[45·4279-5418]/9`）。

- 对标：768_sschro_high → `n=7` 版互补（同式再上一位；772 做高位二对峙）
- 绕行：`[192555-5418]/9=20793` 分子先算后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/771_sschro8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/771_sschro8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/771_sschro8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/771_sschro8/main.sla -o /tmp/basic_771 && /tmp/basic_771
# 期望输出：4279,20793
```
