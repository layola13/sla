# 902 duel_sschro1112

大小 Schröder 续桥二对峙：短递推 vs 半大数（`n=11：2646723` 双锁，`n=12：13648869` 双锁）。

- 对标：901_sschro1112 × 896_schro11 → 续桥源与续高位源合流（792 体例续高位）
- 绕行：半大数 `S(n)/2` 偶数确证（`27297738/2=13648869` 精确整除；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/902_duel_sschro1112/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/902_duel_sschro1112/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/902_duel_sschro1112/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/902_duel_sschro1112/main.sla -o /tmp/basic_902 && /tmp/basic_902
# 期望输出：2646723,2646723,13648869,13648869
```
