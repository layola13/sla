# 878 duel_motz11

Motzkin 续高位三对峙：迭代 vs 直和（`n=10：2188` 双锁，`n=11：5798` 双锁）。

- 对标：876_motz11 × 754_motz_alt910 → 高位源与直和源合流（749 体例续高位）
- 绕行：无（双实现互锁，探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/878_duel_motz11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/878_duel_motz11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/878_duel_motz11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/878_duel_motz11/main.sla -o /tmp/basic_878 && /tmp/basic_878
# 期望输出：2188,2188,5798,5798
```
