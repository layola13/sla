# 857 cat_conv9

Catalan 卷积 `C(9)=sum C(i)C(8-i)=4862`（556 体例续高位）。

- 对标：726_cat_conv6 → 低位卷积版互补（同式再上三位；858 做卷积/商式对峙）
- 绕行：卷积递归 `n=9` 双后端秒级（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/857_cat_conv9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/857_cat_conv9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/857_cat_conv9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/857_cat_conv9/main.sla -o /tmp/basic_857 && /tmp/basic_857
# 期望输出：4862,1430
```
