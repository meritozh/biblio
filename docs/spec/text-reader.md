# Text Reader 实现规格

Feature: `docs/feature/text-reader.md` @ `7077d01bbe30796c87252923cc7e6ca25c725982`

本规格由 `/go-work` 执行。开始或恢复前运行 `git hash-object docs/feature/text-reader.md`，结果须与上方一致。功能变化时停止，由 `to-implement-spec` 更新规格。执行期间用 Git 记录进展。每步一次提交，末行是 `Spec-Step: text-reader#<n>`。最后一步把功能文档改为 `Status: shipped`，用 `git mv` 把本文件移到 `docs/spec/archived/<YYYY-MM-DD>-text-reader.md`，日期取归档当天。

核心层按文件 ID 解析打开方式和本地可读副本，以原文字节为阅读坐标。正文视口、全文查找和章节规则共用解码器，各自持有读取游标。UI 在同一个 NSH 窗口中切页，先显示正文，再接收后台结果。

## Behavior map

测试名是实施时要落地的验收用例。下表的 `open` 指 `commands::text_open`，`download` 指现有下载 worker 和它的队列。`open`、`book`、`rules`、`store` 位于 biblio-core，`navigation`、`viewport`、`controls`、`session` 位于 biblio-ui。表中每项由一个模块决定结果。

| 功能文档行为 | Owner | 通过公开入口验证 |
| --- | --- | --- |
| Who：任意 schema、本地和远端、大小写不敏感的 TXT | `open` | `recognizes_catalog_txt_across_storage_kinds`（自定义 schema、旧远端路径、无扩展加密对象和分片占位路径） |
| Entry：Settings 的单一打开方式覆盖所有 TXT，非 TXT 沿用现有行为 | `open` | `txt_uses_global_opener_and_other_formats_keep_category_opener` |
| Entry：封面／列表／详情页共用选择，右键详情，勾选与收藏，返回原书架位置 | `navigation` | `opens_from_grid_table_group_and_detail_and_restores_shelf`、`selecting_and_favoriting_never_open_a_book` |
| Does：连续滚动、方向键、按屏移动、重排后仍见原段 | `viewport` | `keeps_visible_passage_across_reflow_and_screen_moves` |
| Does：全书文字查找、跨行命中、前后跳转、高亮，阅读页的 Find 快捷键及菜单 | `session` | `finds_literal_across_windows_and_reveals_each_hit`、`find_targets_the_active_reader` |
| Does：跳卷章、首章前的正文、章节字数。零命中仍能读 | `rules` | `outline_preserves_front_matter_volumes_and_counts`、`unmatched_book_remains_readable` |
| Does：两处全局排版控件、即时显示、仅复位四项、跟随应用配色 | `controls` | `settings_and_reader_share_type_and_reset_only_typography` |
| Does：合并连续空行、保留缩进与原换行、可关闭 | `book` | `projects_blank_runs_across_windows_without_changing_source` |
| Does：内置规则、自动检测优先级、各卡命中数、示例、实际选中规则 | `rules` | `surveys_all_presets_and_prefers_explicit_headings` |
| Does：两处进入管理器。预览、取消、本书应用与设为默认各自独立 | `controls` | `preview_cancel_apply_and_set_default_have_distinct_effects` |
| Does：模板及占位符、章／卷正则、示例测试 | `rules` | `template_and_regex_probe_agree_with_outline` |
| Keeps：保护内置规则、自建规则增改删、复制／重命名及两级回退，保留阅读位置 | `store` | `rule_edits_are_atomic_and_deletion_falls_back_on_reopen` |
| Does/Fails：缓存命中、按文件合并下载、失败可重试 | `download` | `uncached_open_joins_download_and_resolves_committed_copy` |
| Does/Fails：等待页显示进度、自动接续，返回后不因迟到结果启动应用 | `navigation` | `retry_after_failure_never_launches_an_abandoned_open` |
| Does：自动解码、候选编码预览、确认后重建正文与目录 | `session` | `encoding_preview_cancel_and_commit_preserve_the_source` |
| Fails：无效正则保留上次有效规则 | `store` | `invalid_edit_keeps_previous_rule_and_default` |
| Fails/Limits：新书／续读先见正文，离开或重选后的旧结果失效，大于样本仍可开 | `session` | `reads_and_leaves_while_indexer_is_blocked_at_50_mib`、`superseded_results_never_replace_current_contents` |
| Keeps：重启续读。全局偏好与按书位置、编码和规则的生命周期 | `store` | `reopen_restores_book_offsets_and_global_preferences` |
| Not this：源文件只读，没有手动书签或自定义主题入口 | `viewport` | `reader_actions_and_theme_stay_within_scope`（交互前后比对源字节） |

现有 `crates/biblio-ui/tests/desktop.rs` 中 `import_search_favorite_edit_and_reopen`、`file_menu_asks_before_deleting` 要改走右键详情，保留原来的编辑、重开与删除断言。`the_shelf_loads_more_files_as_it_scrolls` 改为打开正文后返回，断言第 75 项仍在视口。保留导入采样、分片分派和批量选择的测试。

## Modules

以下声明按 Rustdoc 形式列出，省略函数体。UI 中的 NSH 信号留在 UI 线程，传给服务运行时的是可发送的值。路径从对应 crate 的 `src/` 开始。

### 常见调用

```rust
// 书库卡片在选择模式下切换勾选，否则交给同一个打开入口。
if selection.active.get_untracked() {
    selection.toggle(id);
} else {
    navigation::activate(app, id, Entry::Shelf);
}

// 阅读页与 Settings 挂载同一组控件。只有阅读页有当前正文可预览。
controls::reader_settings(app, Some(session));
controls::reader_settings(app, None);

// 在工作线程取正文的第一个窗口。此时目录任务可以尚未启动。
let book = TextBook::open(file, saved.encoding, &stop)?;
let window = book.window(ReadFrom::Resume(saved.checkpoint), collapse_blanks, &stop)?;
```

### `commands/cache.rs`、`commands/text_open.rs` 与下载队列（修改／新增）

```rust
pub enum Entry { Shelf, DetailOpen }
pub enum OpenTarget {
    Details(i64),                    // non-TXT shelf press
    CategoryFile(i64),               // non-TXT detail Open file
    Reader(i64),
    ExternalTxt { id: i64, program: String },
}
pub async fn target(db: &Library, id: i64, entry: Entry) -> Result<OpenTarget, String>;
pub enum CopyState { Queued, Downloading, Ready(PathBuf), Failed(String) }
pub async fn readable_copy(db: &Library, id: i64)
    -> Result<tokio::sync::watch::Receiver<CopyState>, String>;
pub trait ExternalOpener {
    fn launch(&self, path: &Path, program: Option<&str>) -> Result<(), String>;
}
```

- `target` 是 TXT 识别和路由策略的唯一写法。本地取 `files.path` 的文件名。远端优先取非空 `original_path` 的文件名，兼容历史记录时可取有扩展名的旧远端路径或已记录的友好缓存文件名。只按已知文件名的扩展名做 ASCII 大小写无关比较，不猜书名、schema 或容器类型。可复用 `pipeline/nodes/content_sample.rs` 中现有扩展名判断，将它下沉到无导入依赖的文件类型函数。加密远端对象及分片的真实扩展名不能从对象 ID 推断。
- 非 TXT 的书架单击进详情，详情页的 CategoryFile 直接调用 `cache_open` 的分类应用／系统默认逻辑和原有缺失缓存语义。TXT 的 `ExternalTxt` 总是使用全局显式选择的程序。空程序名在写入外部模式时校验。保留 `cache_open(Library, i64)` 对原调用方的合同，提取其路径解析和 OS 打开实现。TXT 路由和旧调用复用 `load_path_roots`、`cache_to_absolute`、`to_absolute`。
- `readable_copy` 只准备可读路径。打开本地／缓存副本之前检查它是可读文件。缓存记录存在但文件已消失时，对远端类同缓存未命中。本地文件已消失时显示错误。遇到权限错误显示具体原因。就绪的 receiver 初值就是 `Ready`，调用者先读当前值再等待 `changed()`，避免漏掉已经发生的完成。
- `library::Queues` 的下载入口持有按 file ID 合并的在途表，先注册 watch 再投递 `DownloadJob`。读者、详情页和批量下载共享在途任务。显式重新下载仍可在前一任务完成后发起。`download_worker` 继续独占传输和缓存发布，沿用 `NULL` / `bbx1` / `bbx1-split` 分派、解密、拼接与长度校验。生产和测试的对象读取通过 worker 内部的 `#[async_trait::async_trait] trait DownloadObject: Send + Sync { async fn fetch(&self, fs_id: &str, dest: &Path) -> Result<(), String>; }` 注入，认证留在生产适配器内。所有分支返回统一的成功路径或错误，队列负责完成 watch、移除本次在途记录并保留现有 `remote-download-progress` 事件及字段。失败/队列关闭必须唤醒等待者。进度按实际阶段显示，现有事件没有已传字节，不显示虚构百分比。
- 写入同目录临时文件，完整校验后原子替换缓存，再写相对 `local_cache_path`，提交后才发 `Ready`。出错清理本次临时文件，保留已发布完整副本，绝不对外返回半文件。重试重新检查目录记录与磁盘。UI 返回只解除订阅，既有共享下载可继续。`navigation` 检查仍是同一次打开请求才调用 `ExternalOpener::launch` 或打开正文。`open::with_detached` / `that_detached` 只在生产适配器中执行。

### `text_reader/book.rs`（新增）

```rust
pub enum Encoding { Utf8, Utf16Le, Utf16Be, Gbk, Gb18030, Big5 }
pub struct ReadingPosition { pub paragraph: u64, pub restart: u64 }
pub struct TextPoint { pub restart: ReadingPosition, pub utf8_skip: u32 }
pub struct Hit { pub start: TextPoint, pub end: TextPoint }
pub struct Checkpoint { pub at: ReadingPosition, pub source: SourceStamp, pub codec: Encoding }
pub enum ReadFrom { Resume(Option<Checkpoint>), Around(TextPoint), Before(Cursor), After(Cursor) }
pub enum Direction { Previous, Next }
pub struct Window {
    pub rows: Vec<TextRow>, pub before: Option<Cursor>, pub after: Option<Cursor>,
}
impl TextBook {
    pub fn open(file: File, encoding: Option<Encoding>, stop: &CancellationToken)
        -> Result<Self, ReadError>;
    pub fn window(&self, from: ReadFrom, collapse_blanks: bool, stop: &CancellationToken)
        -> Result<Window, ReadError>;
    pub fn find(&self, needle: &str, from: &TextPoint, direction: Direction,
                stop: &CancellationToken) -> Result<Option<Hit>, ReadError>;
    pub fn checkpoint(&self, at: ReadingPosition) -> Checkpoint;
    pub fn sample(&self, candidate: Encoding, stop: &CancellationToken)
        -> Result<String, ReadError>;
}
```

- `TextBook` 绑定同一个只读文件句柄、有效编码和源版本（长度、修改时间，加文件头定长样本摘要供重开核对）。`Cursor` 只在生成它的书和编码中有效。`ReadingPosition.paragraph` 是 BOM 后或逻辑换行后的原文件字节偏移，`restart` 是同段内已确认的字符／解码重启边界，普通段落等于段首。默认 `(0,0)` 表示书首。`TextRow` 包含原文范围、显示文字、与上块是否同段及局部 UTF-8 到源位置的映射。Big5 一个序列产生多个标量时用 `utf8_skip` 区分。数据库里的 `files.progress` 是书目元数据。
- `open` 只读有限文件头（初值 64 KiB）。显式编码优先。自动模式优先 BOM，充分的 UTF-16 特征优先于把含 NUL 的样本当 ASCII。其次检验 UTF-8，再用已有 `charset-normalizer-rs` 的样本判断映射到支持的 `encoding_rs` 解码器。丢掉采样末端不完整的编码单元再判定。候选无把握时使用可预览的 UTF-8 替换显示并提示纠正。UTF-16 在 UI 明示 LE/BE，BOM 不计入正文。空文件正常显示空状态。
- 用独立的定位读取（Unix `FileExt::read_at` / Windows `seek_read`）与有状态增量解码，跨块保留多字节字符、代理对和待判定的 CR。每个查询有自己的游标。同 crate 的规则扫描复用只读迭代器 `pub(super) fn chunks<'a>(&'a self, stop: &'a CancellationToken) -> impl Iterator<Item = Result<TextChunk, ReadError>> + 'a`。每块含原文起止、解码串、换行与延续信息。不要对 `File::try_clone` 共用的文件位置并发 `seek`。`content_sample::sample_text_content` 会整文件读入，不能用作正文路径。
- 一次请求从锚点向前后取有限窗口（初值 64 KiB 源数据块、256 KiB 目标窗口，另带字符边界重叠）。保留 LF、CRLF、CR 的逻辑换行。物理长行返回延续片段，避免一个无换行的 50 MiB 文件形成一个 UI 节点。相邻片段在视口内连续排版。只含空格／Tab 或为空的连续逻辑行投影成一个空白行。前后窗口交界携带空行运行状态。原始字符、缩进和匹配用的原文不变。重开若版本或编码不同，夹紧偏移并重新找经新编码验证的段界，UTF-16 要对齐。在缺少可靠换行的病态长段中，从最近已验证的重启点分块校正并支持取消。
- `find` 是区分大小写的普通文字查找，跨块／逻辑换行延续匹配，向前/向后从当前命中的后一端/前一端继续，到末尾/开头最多环绕一次。空查询清除高亮，零命中可读。只保留当前命中与可见窗口中的匹配范围，反向查找可从检查点顺扫并保留最后一项。正文、查找和目录读取共享同一原文坐标。
- 将同步文件 I/O、解码及检索放在服务运行时的阻塞任务中，每批检查 `CancellationToken`。每个活动阅读会话限制一个当前视口读取、一个目录扫描、一个查找，优先保障视口能运行。过期操作合并，回传使用有界通道或最新值。目录扫描不持有视口所需的锁。`ReadError` 区分 I/O、源变化、失效游标和取消。取消静默，读取失败显示在对应控件并留住已有正文。

### `text_reader/rules.rs`（新增）

```rust
pub enum PatternMode { Template, Regex }
pub struct RuleDraft {
    pub name: String, pub mode: PatternMode, pub chapter: String, pub volume: Option<String>,
}
pub enum Heading { Volume, Chapter }
pub struct RuleSurvey { pub cards: Vec<RuleCard>, pub detected: Option<RuleId> }
impl ChapterRule {
    pub fn compile(draft: &RuleDraft) -> Result<Self, RuleError>;
    pub fn probe(&self, line: &str) -> Result<Option<Heading>, RuleError>;
}
pub fn survey(book: &TextBook, rules: &[SavedRule], stop: &CancellationToken)
    -> Result<RuleSurvey, ScanError>;
pub fn outline(book: &TextBook, rule: &ChapterRule, stop: &CancellationToken)
    -> Result<Contents, ScanError>;
```

- 同一个匹配器服务于编辑器示例、卡片统计与正式目录。模板按整行匹配，普通文字按字面量转义，模板中的空白可匹配空格／Tab，`{序号}` 接受阿拉伯数字、常用中文数字（含零、〇、两及大写数字），`{标题}` 接受非空标题。拒绝未知占位符、空章表达式、换行和无效正则，并定位到具体输入栏。正则输入逐原文逻辑行执行，去行尾换行符，保留段首空白。规则作者可用锚点。用 lock 中的 `fancy-regex` 0.20 系列支持示例里的前后查找，设编译大小和回溯预算（初值 2 MiB、100,000 次）。耗尽预算单独报告该规则错误，保留上次可用目录。卷、章同时匹配时以卷为准。
- 内置 ID 固定，模式和示例由代码提供。`common`（第…章／节）、`classic`（第…回／话）、`english`（Chapter/Section/Part/Episode + 序号）为明确标记组。`digits`（十进制或中文纯数字）、`number-title`（数字加分隔符与标题）、`symbols`（成对特殊符号的标题）为后备组。各预设从实际样本固定正反例。自动检测先选有章命中的明确组，再选后备组，同组取章数多者，同分按上述稳定次序。自建规则参与卡片和手动／全局指定。自动检测限于有已知优先级的内置预设。自动卡显示选中的规则名和章数。
- `survey` 用一次顺序解码遍历统计每条规则，卡片保存计数及首个示例，零命中用规则示例或空态。仅对正在预览／应用的规则生成完整的 `Contents`。目录条目顺序以源位置为准，含类型、标题、源锚点和字数。卷可跳转，独立章数只统计章。首标题前存在文字时加「扉页」，不计入章数。字数按本章正文非空白 Unicode 标量数计算，排除标题，遇下一卷／章或 EOF 结算。目录节点和显示标题按需供给虚拟列表，极长标题按范围延迟提取。后台对超长行分段读取，必要时按行临时拼接以维持整行正则语义，额外内存随最长候选行增长。每批／每条规则之间可取消，任何单次匹配仍受匹配器预算约束。
- `Contents` 属于一次文档版本、规则版本和请求，预览取消即释放。应用预览必须核对当前版本并成功保存选择再替换正式目录。编码确认后废弃旧的正文、搜索和目录结果，从新书对象重算。

### `text_reader/store.rs`（新增）

```rust
pub enum TxtOpener { BuiltIn, External(String) }
pub struct Typography { pub family: Option<String>, pub size: f32, pub leading: f32, pub spacing: f32 }
pub enum BookRule { Inherit, Auto, Fixed(RuleId) }
pub enum DefaultRule { Auto, Fixed(RuleId) }
pub enum Preference { Opener(TxtOpener), Type(Typography), ResetType, Collapse(bool), Default(DefaultRule) }
pub enum RuleEdit {
    Save { id: Option<RuleId>, draft: RuleDraft, make_default: bool },
    Remove(RuleId),
}
pub enum BookEdit { Position(Checkpoint), Encoding(Option<Encoding>), Rule(BookRule) }
impl ReaderStore {
    pub fn new(db: Library) -> Self;
    pub async fn load(&self, file: Option<i64>) -> Result<ReaderState, StoreError>;
    pub async fn preference(&self, change: Preference) -> Result<(), StoreError>;
    pub async fn edit_rule(&self, edit: RuleEdit) -> Result<Option<RuleId>, StoreError>;
    pub async fn edit_book(&self, file: i64, change: BookEdit) -> Result<(), StoreError>;
}
```

`ReaderState` 含全局偏好、可用规则及可选文件的保存状态。`RuleId` 是稳定字符串，内置 `auto` 保留。全局选择只接受 Auto 或 Fixed。`Checkpoint` 同时带 `ReadingPosition`、源版本、最近确认的解码器，使重开能判断是否可直接 seek。

规则先编译后写入，只允许修改/删除自建 ID。复制预设时将其可编辑表达式填入新草稿，再用空 ID 保存。单次操作使用一个事务，返回的 ID 用于定位规则。书级与全局写入均只修改指定字段，防止迟到的位置保存覆盖编码或规则。默认排版用系统字体、18 pt、1.6 倍行高、0 pt 字距，空行合并开启。实现初值与输入范围见 Data。保存失败时保留原有效状态，告知用户并允许重试。


### `reader/navigation.rs`、`reader/session.rs`（新增），`app.rs` 与书架入口（修改）

```rust
// Page 保持 Copy。任务和正文不放进路由枚举。
pub fn activate(app: App, id: i64, entry: Entry);
pub fn screen(app: App, id: i64) -> AnyElement;
pub fn find_on_current_page(app: App);
pub struct ReaderSession { /* NSH 所有者持有的会话句柄 */ }
impl ReaderSession {
    pub fn reveal(&self, point: TextPoint);
    pub fn find(&self, text: String, direction: Direction);
    pub fn confirm_encoding(&self, encoding: Encoding);
}
```

- 加 `Page::Reader(i64)`。发起打开即领取导航序号，离开、换书或重新打开立即使旧序号失效。解析完检查序号，内置目的地进入阅读页并取得可读副本，外部目的地就绪后只启动一次指定应用。对未缓存的外部 TXT 也提供等待／重试／返回状态，完成后回到原入口。启动外部应用失败显示错误并保留返回入口。
- 卡片 `detail::card_face` 接受调用方的 press，`catalog/shelf.rs` 的列表、封面、组内文件、Lucky picks 复用 `activate`。组本身仍进组。选择模式优先切换勾选。收藏和勾选控件消费自己的动作。详情页 `Open file` 使用 `DetailOpen`。网格与列表右键提供 `View details`。列表从 NSH `DataTableContextMenu` 的实际行取 ID，表头和空白处不打开文件。管理页明确要求编辑文件的入口仍进详情。
- `catalog::BrowseState` 已保存分类、查询、过滤、排序、分组、布局和网格首个可见项。沿用 `Paged::new` 的初始加载深度。为表格接上 `on_visible_rows_change`、表格 handle 的 `scroll_to_row`，在已加载到锚点并完成首次布局后恢复，忽略初始化发出的暂时第 0 行。分类、排序、过滤、开组等改变依旧按现有行为清空锚点。返回书库按钮、`mod+[` 与原生 Go 菜单走同一返回路径。
- `app::shortcuts` 和 `chrome` 的 Find 菜单都派发给当前页。阅读中 `mod+f` 聚焦查找框，`mod+k` 保留书库搜索。只在正文拥有键盘焦点时接管箭头、Space、Page Down，编辑器输入空格、按方向键不会滚动正文。
- 会话持有一个正文版本，以及视口、正式目录、规则卡片／预览、查找各自递增的请求序号与取消令牌。先使旧序号失效，再启动替代任务。接收每个结果或错误时核对 owner 仍有效、文件 ID、正文版本和该类序号。确认编码后只提交编码字段，提升正文版本，再取原阅读位置附近的窗口。切换规则只使目录相关任务失效。清理 owner 时注销订阅、取消扫描并交付最新位置。阻塞任务需要主动检查取消令牌，单独 abort JoinHandle 不足以停下扫描。
- 在现有 `run_service` 上调度。`App::job/save` 的全局 busy 会丢弃并行请求且可刷新整个库，阅读和连续设置写入不走该路径。用每窗口一个有序写入队列，排版只合并相邻未执行的同字段修改，位置按书保留最新序号，规则保存／删除／应用按点击顺序作为屏障。全局规则改变时，继承它的当前书立即重算，已固定的书继续使用自身选择。写入成功后才发布已提交版本，失败回退乐观显示或留下可重试错误。滚动停稳或最多每 500 ms 提交最新位置。退页、换书以及窗口正常关闭时强制交付最后一个真实可见锚点。旧会话的收尾必须排在该书下一会话读取之前。为生产退出明确提供队列 drain：由窗口 owner 持有写入者，`src/main.rs` 在 `platform::run_app` 返回后等待该 profile 的已接收写入完成，再报告退出状态。headless 释放窗口走同一收尾。最终写入不依赖已销毁的 UI 回调。数据库错误通过返回值／退出日志可见。

### `reader/viewport.rs`、`reader/controls.rs` 与 `reader/rule_manager.rs`（新增）

```rust
pub fn viewport(session: ReaderSession) -> AnyElement;
pub fn reader_settings(app: App, session: Option<ReaderSession>) -> AnyElement;
pub fn rule_manager(app: App, session: Option<ReaderSession>) -> AnyElement;
pub fn installed_font_families() -> Result<Vec<String>, String>;
```

- 正文使用 NSH 0.4 的 `virtual_list`，数据来自锚点附近的 `Window`。按源位置为行设稳定身份，触及前后边缘时增补窗口、剔除远端窗口。插入或剔除上方行时用原锚点补偿偏移。宽度／排版改变时先记住可见原文位置，等待当前宽度的排版结果再复位。`on_viewable_items_change` 跟踪可见项，`watch_scroll` 或 `on_scroll` 加 `on_layout` 给出实际偏移与视口高度，`ElementRef::scroll_to_offset` 实现按屏翻动。
- 每个普通段落用一个原生 Text，命中片段作为内联 Text 子项，保持原生折行、字体和同一段内的连续排版。空白行占一行高度。命中多屏长段时，先 `scroll_to_index` 挂载，再读 `EventKind::TextLayout` 的行起止（UTF-16）和 `y`，用行内偏移定位到命中。释放已回收行的测量结果。超长单段由有界延续块组成，按原生测量的完整显示行切开，只在可断行处交接，相邻窗口不得产生额外换行、丢字或重复命中。首次从长段中部恢复可先显示有限邻域，后续增补时保持原锚点。单页只保留可见及前后预读的文本和测量记录。字距、行高、字号和字体选择全部作用到正文原生 Text，颜色沿用 `ui::theme`。
- `settings.rs` 加 Reader 标签，与阅读页共用排版控件、复位逻辑和规则管理器。字体首次需要时查询 OS 的已激活字体家族，macOS 用 CoreText、Windows 用 DirectWrite、Linux 用 Pango 的 font map。仅在相应 target 添加依赖，去重排序并缓存。系统字体选项始终可用，已保存字体消失时回落系统字体且允许重选。外部应用控件存平台 opener 接受的程序名或路径。
- 阅读页以当前书进行卡片统计，区分扫描中、零命中与失败。点击卡片只更改预览。取消释放草稿。保存规则先验证再写库，编辑与「设为默认」一起提交时是同一事务。内置预设只能复制。自建可增删改名。删除直接执行 Data 的外键回退。若当前书受到影响，按新的有效规则重算目录并维持正文锚点。Settings 单独打开时仍可管理全局规则、选默认、插入模板占位符并用示例测试，不显示虚构的本书命中数或本书「应用」按钮。规则管理器在小窗口内可滚动。
- 阅读页编码预览读取开头数段，取消不修改书状态。预览、规则编辑和搜索都有自己的输入焦点。`ResetType` 一次恢复四项排版字段，保留其他设置。头部用书名，正文、目录、查找、返回、错误／重试及各设置字段都有可访问名称。

## Data

`database/mod.rs::get_migrations()` 当前最后一项为 7。追加版本 8，可用 `include_str!("text_reader.sql")` 保存以下 SQL。保留版本 1–7 的文本与校验和。规则和引用放在同一 SQLite 数据库，使删除与回退成为一次提交。

```sql
CREATE TABLE reader_rules (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('builtin','custom')),
    name TEXT NOT NULL,
    mode TEXT CHECK (mode IN ('template','regex')),
    chapter TEXT, volume TEXT,
    CHECK ((kind='builtin' AND mode IS NULL AND chapter IS NULL AND volume IS NULL)
        OR (kind='custom' AND mode IS NOT NULL AND chapter IS NOT NULL AND length(chapter)>0))
);
INSERT INTO reader_rules(id,kind,name) VALUES
    ('auto','builtin','自动检测'), ('common','builtin','常规中文'),
    ('classic','builtin','回／话'), ('english','builtin','英文章节'),
    ('digits','builtin','纯数字'), ('number-title','builtin','数字加标题'),
    ('symbols','builtin','特殊符号');

CREATE TABLE reader_preferences (
    id INTEGER PRIMARY KEY CHECK (id=1),
    external_app TEXT CHECK (external_app IS NULL OR length(trim(external_app))>0),
    font_family TEXT, size_pt REAL NOT NULL DEFAULT 18 CHECK (size_pt BETWEEN 8 AND 72),
    line_ratio REAL NOT NULL DEFAULT 1.6 CHECK (line_ratio BETWEEN 1 AND 3),
    spacing_pt REAL NOT NULL DEFAULT 0 CHECK (spacing_pt BETWEEN -1 AND 8),
    collapse_blanks INTEGER NOT NULL DEFAULT 1 CHECK (collapse_blanks IN (0,1)),
    default_rule TEXT REFERENCES reader_rules(id) ON DELETE SET NULL
);
INSERT INTO reader_preferences(id) VALUES (1);

CREATE TABLE reader_books (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    paragraph_byte INTEGER NOT NULL DEFAULT 0 CHECK (paragraph_byte>=0),
    restart_byte INTEGER NOT NULL DEFAULT 0 CHECK (restart_byte>=paragraph_byte),
    source_stamp TEXT, position_codec TEXT,
    encoding TEXT CHECK (encoding IN ('utf8','utf16le','utf16be','gbk','gb18030','big5')),
    rule_id TEXT REFERENCES reader_rules(id) ON DELETE SET NULL
);
CREATE INDEX reader_books_by_rule ON reader_books(rule_id);
```

- `ReaderStore` 是三张表的运行时写入者。内置规则的表达式随程序提供，外键引用稳定 ID。自建 ID 用现有 `uuid` 生成。删除只接受自建 ID。全局 `default_rule IS NULL` 读作自动检测，书级 `rule_id IS NULL` 表示继承，书级 `'auto'` 表示该书明确选择自动检测。删除自建规则时 SQLite 在同一事务中清空所有对应引用，同时覆盖它既是全局默认又被多本书固定的情况。编辑弹窗的「保存并设默认」用同一个写入操作。更新中的失效 ID 返回可重载错误。
- `external_app IS NULL` 表示内置，显式外部模式保存一个非空程序。旧分类的 `view_config.open_app` 原样保留，仍服务非 TXT。无需猜测将多个分类值折成一个全局值。字体名 NULL 表示系统。`source_stamp` 是带版本号的 JSON，包含源长度、mtime 和文件头摘要。`position_codec` 是上次定位时的实际编码，与用户显式覆盖 `encoding` 分开。首次阅读没有书级记录，沿用默认值，首次确认或定位时再按 `file_id` 插入。只写单个变更的列，位置及其 stamp、codec 同写。
- 目录、匹配计数、查找结果、解码正文、视口像素和临时草稿属于会话内派生数据。章节规则删除及缓存清除保留书级记录，删除书库文件触发外键级联。既有 `file_replace` 创建新文件 ID，按新书处理。同一 ID 内容在磁盘上变化时先验证 stamp，再按原文附近合法位置恢复。相同内容搬家或重新下载仍可续读。
- SQLx 以迁移事务添加新表与默认行。失败回滚，不修改旧文件、旧设置或其迁移记录，下次打开可重试。升级测试使用实际 `database::open`，把 `opens_populated_tauri_database_and_preserves_migration_history` 的账本期望扩到 8，验证旧数据、旧 ledger 校验和、外键与二次打开。`commands::test_helpers::setup_db` 只应用 baseline，不可用于证明升级或测试新表。

## Dependencies

| 依赖 | 类别 | 生产适配器 | 测试适配器或验证方式 |
| --- | --- | --- | --- |
| Unicode 解码、正则、章节排序、文字检索 | in-process | `encoding_rs`、`charset-normalizer-rs`、`regex`／`fancy-regex` 共用的匹配器 | 编码字节夹具和规则样例通过 `TextBook` / `ChapterRule` 接口，无网络 |
| 原文与缓存文件 | local-substitutable | 只读定位读取，现有路径解析，原子缓存发布 | 临时目录/真实文件。门控读取适配器用于让扫描停在指定块 |
| SQLite | local-substitutable | `Library::open` 的 SQLx 池，启用外键 | 临时 profile 走完整迁移、重开及失败回滚 |
| 网盘 | true external | 下载 worker 内部引入 `DownloadObject::fetch(fs_id, dest)`。生产适配器使用 `ensure_access_token`、`get_download_dlink` 和 `download_to` | 同一 worker 注入可暂停、报错、返回原始或加密对象的适配器，验证真实解密、拼接及入库 |
| 外部应用 | true external | `ExternalOpener` 调用系统 opener | 记录程序、路径及调用次数。模拟启动失败 |
| NSH 渲染／字体 | local-substitutable | 同一 `biblio_ui::root` 与系统文字引擎，按平台查询已安装字体 | `nsh_testing::Headless`、固定字体列表。原生窗口补验实际度量与滚动 |

基于现有 Tokio 运行时，在直接使用处声明 `tokio-util` 的 `CancellationToken` 依赖并更新锁文件。测试控制执行顺序只在确实有竞态的 I/O 或任务调度处注入门闩。纯默认值、坐标、规则打分无需 trait。

## Steps

每步带本规格的 `Spec-Step` trailer。下列 Check 是针对该步的检查，提交前还要通过仓库的 `cargo test --locked --workspace`。用临时 profile。所有测试通过实现的调用接口进入。

1. **迁移阅读器状态。** After: none. 在 `database/mod.rs` 追加 v8 和新 SQL 文件，升级旧库并验证回滚、重复打开、两级外键回退与文件级联。Check: `cargo test --locked -p biblio-core --lib database::tests`。
2. **建立只读正文和原文坐标。** After: none. 在 `biblio-core/src/text_reader/{mod,book}.rs` 定义位置、增量解码、有界窗口、空行投影、示例解码及文字查找，导出核心模块。所需依赖／锁文件在此提交。测跨块 CRLF、多字节断点、Big5 多标量、UTF-16 代理对、原文不变与无换行长段。Check: `cargo test --locked -p biblio-core --lib text_reader::book`。
3. **实现章节规则、统计与目录。** After: 2. 写 `text_reader/rules.rs` 并连接上一步的正文游标。覆盖所有预设、模板与正则同源测试、卷章冲突、零命中、预算错误、扉页及字数。Check: `cargo test --locked -p biblio-core --lib text_reader::rules`。
4. **持久化全局偏好与按书选择。** After: 1, 2, 3. 实现 `store.rs` 和 ReaderState 等值类型，编译有效规则后事务提交。覆盖交错写、重开、删除全局兼本书规则、原位置与元数据保留。Check: `cargo test --locked -p biblio-core --lib text_reader::store`。
5. **使已有下载提供可等待的本地副本。** After: none. 修改 `library.rs`、`services/download_worker.rs`、`commands/{remote,cache}.rs`，共享在途任务、发布阶段和结果。保留已有批量接口及事件兼容。用现有依赖完成此步，测试失败后重试、离开并重入、缓存被移走和三种容器分派。Check: `cargo test --locked -p biblio-core --lib download`。
6. **统一 TXT 与既有文件的打开路由。** After: 2, 4, 5. 接入 `commands/text_open.rs` 和共享的可读路径／外部应用调用。为书架点击与详情打开给出确定结果。覆盖本地、加密远端、未缓存、外部程序错误及非 TXT 原行为。Check: `cargo test --locked -p biblio-core --lib text_open`。
7. **接入正文视口与阅读会话。** After: 2, 3, 4, 6. 新增 `biblio-ui/src/reader/`，接入 App 路由、详情页 TXT 打开和正常退出 drain。落地连续滚动、原位续读、下载等待与重试、查找及原生菜单分流。用实际详情入口或可控会话运行大文件并发用例，再测关闭时落盘。Check: `cargo test --locked -p biblio-ui`。
8. **连接全局设置、编码预览和章节规则管理器。** After: 3, 4, 7. 阅读页与 Settings 复用控件，接入系统字体适配器及目标平台依赖，在同一提交更新锁文件。验收即时排版、规则卡片／目录预览、两种编辑模式、保存／取消、删除回退、编码确认与旧结果失效。Check: `cargo test --locked -p biblio-ui`。
9. **书架单击开读并恢复浏览位置。** After: 6, 7, 8. 接入网格、列表、组内文件与 Lucky picks，完成右键、批选及列表锚点恢复。同步更新前述三条桌面测试，保留非 TXT 和元数据编辑原行为。Check: `cargo test --locked -p biblio-ui --test desktop`。
10. **验收并归档。** After: 1, 2, 3, 4, 5, 6, 7, 8, 9. 在独立 profile 跑以下验收，达标后在同一提交把功能状态改为 shipped 并用 `git mv` 归档。Check: `cargo check --locked --workspace --all-targets`、`cargo test --locked --workspace`、`scripts/bundle-macos.sh release` 及原生阅读验收。

### 最后一步的验收

- 用临时目录生成准确 50 MiB、含已知标记及章节的 TXT，另测更大的输入和无换行长段。确定性测试用门闩挂起目录扫描：首屏与下半本续读均可在释放门闩前显示正确原文。滚动一屏、按返回并打开另一本书，释放门闩后旧结果不能回到界面。反向完成两次预览，验证晚到结果不能覆盖新选项。记录结果以正文标记、已返回页和目录身份为准。
- 在同源码构建的原生 macOS 包，用隔离的 `BIBLIO_CONFIG_DIR` 打开相同样本。分别检查原生滚轮、Space/Page Down、增减字号和改变宽度后同段仍可见、查找跨屏长段命中高亮、下载失败后返回、规则预览后取消／应用，以及滚至中段立即正常退出后重开。验证 Settings 中能选择本机实际字体。头部菜单与键盘都测一次 Find。保存提交号、输入大小、预期标记和实测结果于验收记录或实施提交说明。耗时如有记录，注明设备和构建模式。
- 网络逻辑由可控传输覆盖。实际网盘连通性仅在具备授权测试条件并真实验证时报告。系统字体、原生排版和滚动表现以原生窗口实测为准。

## Not this

阅读器只读 TXT。在既有书库模型上增加阅读状态，无需迁移或重写原文。不建立全文镜像、按书打开应用的配置或持久化派生目录。手动书签、自定义主题和原文编辑不设存储或入口。
