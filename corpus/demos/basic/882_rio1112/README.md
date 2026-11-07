# 882 rio1112

Riordan 续高位二 `R(11)=1585`，`R(12)=4213`（短递推续高位；752 体例续）。

- 对标：752_rio910 → `n=9/10` 版互补（短递推逐项精确整除；883 做续桥对峙）
- 绕行：`11·(2·603+3·232)/13=1585` 递推手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/882_rio1112/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/882_rio1112/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/882_rio1112/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/882_rio1112/main.sla -o /tmp/basic_882 && /tmp/basic_882
# 期望输出：1585,4213,603
```
