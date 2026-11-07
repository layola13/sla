# 409 rev num

整数反转（`12345 → 54321`；234 的反转版）。

- 对标：234 numpalin → 余数拼装（`100 → 1` 尾零丢弃属十进制语义）
- 绕行：无（`m > 0` 守卫；纯 `int` 循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/409_rev_num/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/409_rev_num/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/409_rev_num/main.sla -o /tmp/basic_409 && /tmp/basic_409
# 期望输出：54321,1,7
```
