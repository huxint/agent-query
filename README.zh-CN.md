<p align="center">
  <a href="./README.md">English</a> | <strong>中文</strong>
</p>

# agent-query

`agent-query` 是一个给支持本地 skills 的 agent 用的仓库分析 skill。

这个仓库提供 `skills/agent-query/` 里的可安装 skill 包。这个包里已经附带了预编译好的 `agent-query` 可执行文件，以及指导 agent 何时使用它的 skill 说明。

## 为什么要用它

模型在拿到正确上下文之后，往往能做出不错的判断；但在刚进入一个中大型仓库时，第一轮摸图通常并不稳定。

如果没有结构化工具，agent 很容易：

- 随机打开文件
- 只靠文件名和目录名猜架构
- 漏掉真正的入口和核心模块
- 还没搞清依赖关系就开始改边缘代码
- 把大量上下文浪费在“先搞懂仓库”这件事上

`agent-query` 的作用，就是先把这层结构信息补上。

## 这个 skill 能带来什么

- 一个可直接安装到支持本地 skills 的 agent 里的 skill 包。
- 一个随 skill 一起提供的仓库分析工具。
- 在修改、review 或规划任务之前，先拿到更好的仓库上下文。
- 更快回答结构、依赖、符号、热点和摘要这类问题。

## 安装

先 clone 仓库：

```bash
git clone https://github.com/huxint/agent-query.git
cd agent-query
```

把 skill 复制到你的 agent skills 目录：

```bash
mkdir -p "$AGENT_SKILLS_DIR"
cp -R skills/agent-query "$AGENT_SKILLS_DIR/agent-query"
```

如果你的环境兼容 Codex 的默认路径，对应位置是：

```bash
mkdir -p ~/.codex/skills
cp -R skills/agent-query ~/.codex/skills/agent-query
```

## 如何触发

具体语法取决于你使用的 agent。

- 在 Codex 风格的环境里，可以用 `$agent-query` 调用。
- 在其他 agent 里，用该 agent 自己的 skill 语法按名称调用即可。

常见请求方式：

- `用 agent-query 先分析这个仓库，再开始改代码。`
- `用 agent-query 告诉我应该先读哪些文件。`
- `用 agent-query 概括一下这个仓库的整体结构。`
- `用 agent-query 看一下这个模块周围的依赖关系。`

## 适合什么场景

- 仓库已经大到手动翻文件又慢又吵。
- 你希望 agent 按结构而不是按猜测来选择文件。
- 你希望在改代码或做 review 之前，先补一层可复用的结构上下文。

## 说明

这个仓库主要是用来提供 skill 包的，不是完整的 CLI 使用手册。
