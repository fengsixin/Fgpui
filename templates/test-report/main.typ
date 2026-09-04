// 测试报告模板入口。数据契约：schema.json；示例：examples/sample.json
#let data = json("data.json")
#import "theme.typ": apply-theme
#import "components/ui.typ": meta-table

#show: apply-theme

#align(center)[
  #text(size: 20pt, weight: "bold")[#data.title]
]

#v(0.8em)
#meta-table((
  ([*报告标题*], [#data.title]),
  ([*测试负责人*], [#data.at("author", default: "")]),
  ([*报告版本*], [#data.at("version", default: "")]),
  ([*报告日期*], [#data.at("date", default: "")]),
  ([*密级*], [#data.at("classification", default: "")]),
))

#if data.at("scope", default: "") != "" [
  == 测试范围
  #data.scope
]

#let env = data.at("environment", default: none)
#if env != none [
  == 测试环境
  #if env.at("os", default: "") != "" [- 操作系统：#env.os]
  #let tools = env.at("tools", default: ())
  #if tools.len() > 0 [- 测试工具：#tools.join("、")]
  #if env.at("notes", default: "") != "" [#env.notes]
]

#let cases = data.at("cases", default: ())
#if cases.len() > 0 [
  == 测试用例
  #table(
    columns: 5,
    inset: 6pt,
    stroke: 0.5pt + luma(180),
    table.header([*编号*], [*名称*], [*预期结果*], [*实际结果*], [*状态*]),
    ..cases.map(c => (
      [#c.at("id", default: "")],
      [#c.at("name", default: "")],
      [#c.at("expected", default: "")],
      [#c.at("actual", default: "")],
      [#c.at("status", default: "")],
    )).flatten(),
  )
]

#let issues = data.at("issues", default: ())
#if issues.len() > 0 [
  == 问题清单
  #table(
    columns: 4,
    inset: 6pt,
    stroke: 0.5pt + luma(180),
    table.header([*编号*], [*严重程度*], [*问题描述*], [*处理状态*]),
    ..issues.map(i => (
      [#i.at("id", default: "")],
      [#i.at("severity", default: "")],
      [#i.at("summary", default: "")],
      [#i.at("status", default: "")],
    )).flatten(),
  )
]

== 测试结论
#data.conclusion
