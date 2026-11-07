# 874 fub10

有序 Bell 再高位 `F(10)=102247563`（同式再上一位；738 体例高位）。

- 对标：866_fub9 → `n=9` 版互补（递推扇出 `n=10` 双后端秒级；875 做三向收束）
- 绕行：`C(10,5)=252` 系数源连验（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/874_fub10/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/874_fub10/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/874_fub10/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/874_fub10/main.sla -o /tmp/basic_874 && /tmp/basic_874
# 期望输出：102247563,7087261
```
