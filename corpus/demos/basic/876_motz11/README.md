# 876 motz11

Motzkin 续高位三 `M(11)=5798`（`M(10)=2188` 对照；卷积和 `3610`）。

- 对标：748_motz10 → `n=10` 版互补（同式再上一位；878 做迭代/直和对峙）
- 绕行：`5798-2188=3610` 卷积和手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/876_motz11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/876_motz11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/876_motz11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/876_motz11/main.sla -o /tmp/basic_876 && /tmp/basic_876
# 期望输出：5798,2188
```
