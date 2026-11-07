# 725 tri_nar2

Narayana 三向：行和 × Catalan 商式 × Catalan 卷积（`n=7：429 三锁`）。

- 对标：717_nar_row7 → 行源互补（605 三向体例；三算法互锁收束 Narayana 续高位族）
- 绕行：卷积递归 8s 级（探针 sa 8.2s；`nsum(6)=132` 低位陪锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/725_tri_nar2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/725_tri_nar2/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/725_tri_nar2/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/725_tri_nar2/main.sla -o /tmp/basic_725 && /tmp/basic_725
# 期望输出：429,429,429
```
