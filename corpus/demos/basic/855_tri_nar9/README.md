# 855 tri_nar9

Narayana 续三向：行和 × Catalan 商式 × Catalan 卷积（`n=9：4862` 三锁）。

- 对标：847_nar_row9 → 行源互补（三实现互锁；725 三向体例收束 Narayana 续三行族）
- 绕行：卷积递归 `n=9` 双后端秒级（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/855_tri_nar9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/855_tri_nar9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/855_tri_nar9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/855_tri_nar9/main.sla -o /tmp/basic_855 && /tmp/basic_855
# 期望输出：4862,4862,4862
```
