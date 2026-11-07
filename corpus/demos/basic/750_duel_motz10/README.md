# 750 duel_motz10

Motzkin 续高位二对峙：迭代 vs 直和（`n=9：835 对 835`；`n=10：2188 对 2188`）。

- 对标：748_motz10 → 高位源互补（749 对峙体例上移；双边同值锁定）
- 绕行：直和六项 `1+45+420+1050+630+42=2188` 精确求和（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/750_duel_motz10/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/750_duel_motz10/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/750_duel_motz10/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/750_duel_motz10/main.sla -o /tmp/basic_750 && /tmp/basic_750
# 期望输出：835,835,2188,2188
```
