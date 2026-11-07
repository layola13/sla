# 915 tri_bell11

Bell 续三向二：二项递推 × Aitken 三角 × 二类行和（`n=11：678570` 三锁）。

- 对标：906_bell11 → 递推源互补（三实现互锁；785 三向体例收束 Bell 续三行族）
- 绕行：三实现皆探针三线绿（递推/三角/行和秒级会师）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/915_tri_bell11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/915_tri_bell11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/915_tri_bell11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/915_tri_bell11/main.sla -o /tmp/basic_915 && /tmp/basic_915
# 期望输出：678570,678570,678570
```
