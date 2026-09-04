// 测试报告模板入口。数据契约：schema.json；示例：examples/sample.json
// 结构：封面 → 目录（页眉页脚/页码）→ 修订记录 → 范围/环境/用例/问题 → 结论 → 补充正文
#let data = json("data.json")
#import "theme.typ": apply-theme
#import "components/ui.typ": meta-table, revisions-table

#show: apply-theme

// 封面（无页码）
#set page(numbering: none, header: none, footer: none)
#align(center + horizon)[
  #v(-2.5cm)
  #text(size: 24pt, weight: "bold")[#data.title]
  #v(1.6em)
  #meta-table((
    ([*测试负责人*], [#data.at("author", default: "")]),
    ([*报告版本*], [#data.at("version", default: "")]),
    ([*报告日期*], [#data.at("date", default: "")]),
    ([*密级*], [#data.at("classification", default: "")]),
  ))
]

// 正文区：页眉与页脚
#set page(
  numbering: "1",
  header: align(right)[#text(size: 9pt, fill: luma(120))[#data.title ｜ 密级：#data.at("classification", default: "")]],
  footer: context align(center)[#text(size: 9pt)[第 #counter(page).display() 页 / 共 #counter(page).final().first() 页]],
)

#pagebreak()
#outline(title: [目录], depth: 3)
#pagebreak()

#revisions-table(data.at("revisions", default: ()))

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

#let extra = data.at("body", default: "")
#if extra != "" [
  == 补充说明
  #eval(extra, mode: "markup")
]
