# 863 cat_ratio9

Catalan 比值递推高位 `C(9)=4862`，`C(10)=16796`，`C(11)=58786`（逐轮精确整除）。

- 对标：733_cat_ratio → 低位比值版互补（同式再上高位；865 做三向收束）
- 绕行：比值递推逐轮精确整除（`48620/10`、`184756/11`、`705432/12`；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/863_cat_ratio9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/863_cat_ratio9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/863_cat_ratio9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/863_cat_ratio9/main.sla -o /tmp/basic_863 && /tmp/basic_863
# 期望输出：4862,16796,58786
```
