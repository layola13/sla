# 197 Profile Guided Opt

This slot keeps profile-guided optimization observable as a hot-hot-hot-cold call mix totaling `10`.

- `main.rs`: Rust reference for the hot-call profile mix.
- `main.sla`: Sla companion for the hot-call profile mix.

Commands:

```bash
SA_PLUGIN_DEV=1 sa sla check demos/rosetta/197_profile_guided_opt/main.sla
SA_PLUGIN_DEV=1 sa sla build demos/rosetta/197_profile_guided_opt/main.sla --out /tmp/197_profile_guided_opt.sa
SA_PLUGIN_DEV=1 sa sla test demos/rosetta/197_profile_guided_opt/main.sla
```
