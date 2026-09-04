// 技术方案模板入口。数据契约：schema.json；示例：examples/sample.json
#let data = json("data.json")
#import "theme.typ": apply-theme
#import "components/ui.typ": meta-table, chapter-table, figure-block

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

#if data.at("background", default: "") != "" [
  == 背景与目标
  #data.background
]

#for chapter in data.at("chapters", default: ()) [
  #heading(level: chapter.at("level", default: 2), chapter.at("heading", default: ""))
  #if chapter.at("content", default: "") != "" [#chapter.content]
  #if chapter.at("table", default: none) != none [#chapter-table(chapter.table)]
  #for img in chapter.at("images", default: ()) [#figure-block(img)]
  #v(0.6em)
]
