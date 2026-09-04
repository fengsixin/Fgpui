// 技术方案模板入口。数据契约：schema.json；示例：examples/sample.json
// 正文（data.body）为 Typst 标记源码，用户自由书写，经 eval 渲染。
#let data = json("data.json")
#import "theme.typ": apply-theme
#import "components/ui.typ": meta-table

#show: apply-theme

#align(center)[
  #text(size: 20pt, weight: "bold")[#data.title]
  #if data.at("subtitle", default: "") != "" [
    #v(0.4em)
    #text(size: 12pt, fill: luma(100))[#data.subtitle]
  ]
]

#v(0.8em)
#meta-table((
  ([*文档标题*], [#data.title]),
  ([*作者 / 部门*], [#data.at("author", default: "")]),
  ([*文档版本*], [#data.at("version", default: "")]),
  ([*编写日期*], [#data.at("date", default: "")]),
  ([*密级*], [#data.at("classification", default: "")]),
))

#if data.at("summary", default: "") != "" [
  == 摘要
  #data.summary
]

#let body-src = data.at("body", default: "")
#if body-src != "" [
  #eval(body-src, mode: "markup")
]
