# 867 fubw9

有序 Bell 加权和 `F(9)=sum k!S(9,k)=7087261`（741 体例续高位）。

- 对标：866_fub9 → 递推版互补（同式加权复算；873 做表加对峙，875 做三向）
- 绕行：加权分项十项手算复核（`2328480+1451520+362880` 尾三项；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/867_fubw9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/867_fubw9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/867_fubw9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/867_fubw9/main.sla -o /tmp/basic_867 && /tmp/basic_867
# 期望输出：7087261,545835
```
