# 901 sschro1112

小 Schröder 续高位二 `s(11)=2646723`，`s(12)=13648869`（短递推续高位；791 体例续）。

- 对标：791_sschro910 → `n=9/10` 版互补（短递推逐项精确整除；902 做续桥对峙）
- 绕行：`[69·2646723-10·518859]/13=13648869` 递推手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/901_sschro1112/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/901_sschro1112/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/901_sschro1112/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/901_sschro1112/main.sla -o /tmp/basic_901 && /tmp/basic_901
# 期望输出：2646723,13648869,518859
```
