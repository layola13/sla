# 593 fubini6

Fubini 高位 `F(6)=4683`（行和式直算：`1+62+540+1560+1800+720`）。

- 对标：588_fubini → 低位版互补（同式高位复算，大值单锁；另断言 `F(5)=541` 承接）
- 绕行：分项和手算复核（`S(6,*)=1,31,90,65,15,1`，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/593_fubini6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/593_fubini6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/593_fubini6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/593_fubini6/main.sla -o /tmp/basic_593 && /tmp/basic_593
# 期望输出：4683
```
