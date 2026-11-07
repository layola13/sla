# 754 motz_alt910

Motzkin 直和高位 `M(9)=835`，`M(10)=2188`（633 体例续高位；同式再上两位）。

- 对标：633_motz_closed → 低位直和版互补（同式再上三位；755 做三向收束）
- 绕行：直和六项 `1+45+420+1050+630+42=2188` 精确求和（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/754_motz_alt910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/754_motz_alt910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/754_motz_alt910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/754_motz_alt910/main.sla -o /tmp/basic_754 && /tmp/basic_754
# 期望输出：835,2188
```
