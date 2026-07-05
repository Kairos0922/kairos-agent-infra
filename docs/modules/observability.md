# observability 模块设计

## 0. 定位与边界

Step 的持久化与查询是本模块唯一职责。三个消费方:
① loop(写入,checkpoint 语义)② SSE 断线补发与 run 回放
③ eval 回放与 distill 管线(读取)。
**不归本模块**:UsageRecord(归 model_gateway,§S7)、
应用日志(归 foundation/logging)、指标看板(暂缓,见 §7)。
注意:trace 存储不是"日志"——它是受访问控制的结构化数据,
服务端持有全文;"日志不落明文"的安全约定管的是 logging,
不适用于本模块(此区别写入安全文档,避免误解)。

## 1. 契约(contracts/)

```python
class StepSink(Protocol):
    async def append(self, ctx: TenantContext, step: Step) -> None
    # loop 的 checkpoint 依赖:append 成功才进下一轮(S4 已定)
    # 幂等:同 (run_id, agent_path, turn) 重复写入覆盖而非报错
    #  (恢复场景会重放最后一轮)

class TraceQuery(Protocol):
    async def get_run(self, ctx, run_id) -> RunRecord        # run 级汇总
    async def list_runs(self, ctx, filter, page) -> Page[RunRecord]
    # filter: user/profile/status/时间窗(distill 与控制台的消费面)
    async def get_steps(self, ctx, run_id) -> list[Step]     # 回放/eval
```

## 2. 存储布局

- 关系库,与 SessionStore 同库(dev SQLite / 生产 PostgreSQL),
  不为 trace 单独引存储(YAGNI)。
- 两张表:runs(run 级汇总:status/usage/profile_ref/起止时间,
  run_finished 时落)、steps(Step 全文,JSON 列)。
- 租户隔离:tenant_id 列 + 复合索引 + 查询强制过滤
  (provider 层内注入,同 memory 的 user 级过滤模式);
  租户注销 = 按 tenant_id 批量删除(关系库天然支持,
  无需物理分表)。
- 大字段上限:Step 内单字段(工具结果/模型输出)上限 256KB,
  超限截断并打 truncated 标记(头尾保留);工具全文本来就有
  "重新调用可再取"的语义(S5 §3),不为极端大结果引对象存储。

## 3. 事件补发与回放(与 S3 协议的接缝)

两条路径,职责分明:
- **活跃 run 的断线补发**:server 侧每活跃 run 维护内存环形
  缓冲(默认 1000 条事件),Last-Event-ID 从缓冲补发。
  进程重启缓冲即失,客户端重连后走↓
- **已结束/缓冲缺失的回放**:由 Step 序列重建事件流。
  重建是有损的:text_delta 合并为整段文本(增量粒度不持久化,
  S3 已定"不双写明细")。对"看结果"足够,对"重放打字机效果"
  不支持——明确接受此取舍。

## 4. 保留期与降采样

- steps 明细在线保留 30 天(租户可配);到期定时任务删除明细,
  runs 汇总长期保留。
- distill 消费窗口(默认近 7 天)必须 ≤ 保留期,配置校验。

## 5. OTel 导出(可选开关)

OtelExporter 作为 StepSink 的装饰器实现:append 时同步映射
run→trace / step→span / 工具调用→child span,推 OTLP 端点。
默认关闭;行业客户有既有监控体系时打开。不引 OTel 做主存储
(查询契约不依赖它)。

## 6. 访问控制

TraceQuery 全部方法带 ctx,租户内可见;跨租户的运维查询
(排障)走 server 管理面单独端点,要求管理凭据并记审计日志
(端点归 S15 登记)。

## 7. 暂缓
指标看板(先用 OTel 导出 + 客户自有 Grafana) │ trace 采样
(全量写,量大再说) │ 对象存储 offload │ text_delta 粒度持久化