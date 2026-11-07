# 871 fubw9d

有序 Bell 加权分项 `7!S(9,7)=2328480`，`8!S(9,8)=1451520`，`9!S(9,9)=362880`（尾三项锁定）。

- 对标：867_fubw9 → 总量版互补（分项直打；十项和 `7087261` 会师）
- 绕行：`5040·462=2328480` 分项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/871_fubw9d/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/871_fubw9d/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/871_fubw9d/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/871_fubw9d/main.sla -o /tmp/basic_871 && /tmp/basic_871
# 期望输出：2328480,1451520,362880
```
