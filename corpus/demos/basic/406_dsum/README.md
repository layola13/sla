# 406 dsum

数位和（`12345 → 15`；342 的求和版）。

- 对标：342 droot → 取模整除循环（`999 → 27`）
- 绕行：无（`m > 0` 守卫；`0` 输入得 0）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/406_dsum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/406_dsum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/406_dsum/main.sla -o /tmp/basic_406 && /tmp/basic_406
# 期望输出：15,27,7
```
