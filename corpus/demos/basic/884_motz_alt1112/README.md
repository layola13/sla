# 884 motz_alt1112

Motzkin 直和再高位 `M(11)=5798`，`M(12)=15511`（754 体例续高位；同式再上两位）。

- 对标：754_motz_alt910 → 低位直和版互补（直和六项手算复核；885 做三向收束）
- 绕行：`C(12,2k)C(2k,k)/(k+1)` 七项逐项精确整除（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/884_motz_alt1112/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/884_motz_alt1112/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/884_motz_alt1112/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/884_motz_alt1112/main.sla -o /tmp/basic_884 && /tmp/basic_884
# 期望输出：5798,15511,2188
```
