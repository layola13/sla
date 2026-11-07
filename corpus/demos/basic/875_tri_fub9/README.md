# 875 tri_fub9

有序 Bell 续三向：商版递推 × 加权和 × 表版递推（`n=9：7087261` 三锁）。

- 对标：866_fub9 → 商源互补（三实现互锁；745 三向体例收束 Fubini 续三行族）
- 绕行：三实现皆探针三线绿（递推/加权/表版秒级会师）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/875_tri_fub9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/875_tri_fub9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/875_tri_fub9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/875_tri_fub9/main.sla -o /tmp/basic_875 && /tmp/basic_875
# 期望输出：7087261,7087261,7087261
```
