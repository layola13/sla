# 619 delannoy_sym

Delannoy 对称性：`D(5,3)=D(3,5)=231`，`D(5,4)=D(4,5)=681`（转置双算一致）。

- 对标：611_delannoy_tab → 建表版互补（行列镜像互锁；601 对称体例）
- 绕行：转置对调用各自独立建表（表形不同 `w` 各异，双边同值锁定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/619_delannoy_sym/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/619_delannoy_sym/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/619_delannoy_sym/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/619_delannoy_sym/main.sla -o /tmp/basic_619 && /tmp/basic_619
# 期望输出：231,231,681,681
```
