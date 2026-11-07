# 879 motz12

Motzkin 续高位四 `M(12)=15511`（`M(11)=5798` 对照；卷积和 `9713`）。

- 对标：876_motz11 → `n=11` 版互补（同式再上一位；880 做高位对峙）
- 绕行：`15511-5798=9713` 卷积和手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/879_motz12/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/879_motz12/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/879_motz12/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/879_motz12/main.sla -o /tmp/basic_879 && /tmp/basic_879
# 期望输出：15511,5798
```
