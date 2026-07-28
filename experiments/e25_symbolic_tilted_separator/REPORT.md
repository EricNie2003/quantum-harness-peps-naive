# E25：symbolic tilted separator / bottom-v2 automaton

## 决策

**DEPENDENCY REJECT；没有启动实现或 benchmark。**

预注册规则要求 E21 或 E23 至少一个通过其 production keep gate 才允许
启动 E25。E21 的 canonical DD 虽然正确且可作为 E22 研究基线，但连续
N=8/9 的 nodes/support 为 4.53/3.65，触发 `>0.8` kill gate，并比 direct
D4 慢 59--71 倍。E23 的 proportional edge quotient 在 N=8--10 只减少
6.1%、14.6%、5.4% nodes，并增加 26--46% 时间，也被拒绝。

E22 保留的是“用 actual node count 搜索变量顺序”的机制，不是可用的
symbolic production representation；它不能替代 E25 明确指定的 E21/E23
成功前置条件。

因此没有创建实验 worktree、没有编写 bottom automaton，也没有重跑 E20
的 concrete bottom support。这样避免在已知会触发 `4^N`/bottom-v2
materialization 的表示上消耗资源或把依赖失败误报为新优化尝试。

该 dependency rejection 计入 E21--E25 五方向周期，随后必须完成强制
review 才能启动 E26。
