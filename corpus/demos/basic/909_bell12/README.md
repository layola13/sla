# 909 bell12

Bell 续高位四 `B(12)=4213597`（二项递推，同式再上一位）。

- 对标：906_bell11 → `n=11` 版互补（同式再上一位；910 做高位对峙）
- 绕行：`4213597-678570=3535027` 差手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/909_bell12/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/909_bell12/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/909_bell12/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/909_bell12/main.sla -o /tmp/basic_909 && /tmp/basic_909
# 期望输出：4213597,678570
```
