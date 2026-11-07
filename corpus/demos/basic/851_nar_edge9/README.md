# 851 nar_edge9

Narayana 边值 `N(9,0)=1`，`N(9,8)=1`，`N(9,1)=36=C(9,2)`（首尾恒 1 + 边恒等）。

- 对标：722_nar_edge → 低位边值互补（边恒等 `N(n,1)=C(n,2)` 双值连验；855 做三向收束）
- 绕行：`9·8/2=36` 三角数手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/851_nar_edge9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/851_nar_edge9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/851_nar_edge9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/851_nar_edge9/main.sla -o /tmp/basic_851 && /tmp/basic_851
# 期望输出：1,1,36
```
