# 726 cat_conv6

Catalan 卷积 `C(n+1)=sum C(i)·C(n-i)`（`C(5)=42`，`C(6)=132`；556 体例续高位）。

- 对标：556_cat_conv → 低位卷积版互补（同式再上两位；728 做卷积/商式对峙）
- 绕行：卷积递归 8s 级（探针 sab 5.5s / sa 8.3s 含全组断言）；`C(4)=14` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/726_cat_conv6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/726_cat_conv6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/726_cat_conv6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/726_cat_conv6/main.sla -o /tmp/basic_726 && /tmp/basic_726
# 期望输出：42,132
```
